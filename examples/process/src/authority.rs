//! Process room authority.
//!
//! Wraps a running subprocess as an Interconnect room. Intents become stdin
//! input; stdout/stderr become snapshot lines.

use crate::protocol::{ProcessIntent, ProcessPassport, ProcessSnapshot};
use interconnect_core::{ImportResult, Session, SimpleAuthority};
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

const MAX_LINES: usize = 200;

struct ProcessState {
    lines: Vec<String>,
    running: bool,
    exit_code: Option<i32>,
}

/// A room authority that wraps a running subprocess.
pub struct ProcessAuthority {
    /// Display label for the command.
    command: String,
    /// Channel to send text lines to the process's stdin.
    stdin_tx: mpsc::UnboundedSender<String>,
    /// Shared output state, updated by background I/O tasks.
    state: Arc<Mutex<ProcessState>>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("process has exited")]
    NotRunning,
    #[error("stdin send failed: {0}")]
    SendFailed(String),
}

impl ProcessAuthority {
    /// Spawn a subprocess and create an authority for it.
    ///
    /// `update_tx` is signalled whenever new output arrives so the server can
    /// broadcast an updated snapshot to all connected clients.
    pub async fn spawn(
        command: &str,
        args: &[String],
        update_tx: mpsc::UnboundedSender<()>,
    ) -> std::io::Result<Self> {
        let mut child = tokio::process::Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let child_stdin = child.stdin.take().expect("stdin pipe");
        let child_stdout = child.stdout.take().expect("stdout pipe");
        let child_stderr = child.stderr.take().expect("stderr pipe");

        let state = Arc::new(Mutex::new(ProcessState {
            lines: Vec::new(),
            running: true,
            exit_code: None,
        }));

        let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<String>();

        // Forward stdin channel → process stdin
        tokio::spawn(async move {
            let mut stdin = child_stdin;
            while let Some(line) = stdin_rx.recv().await {
                if stdin.write_all(line.as_bytes()).await.is_err() {
                    break;
                }
                if stdin.write_all(b"\n").await.is_err() {
                    break;
                }
                let _ = stdin.flush().await;
            }
        });

        // Read stdout → shared state
        {
            let state = state.clone();
            let update_tx = update_tx.clone();
            tokio::spawn(async move {
                use tokio::io::AsyncBufReadExt;
                let mut lines = tokio::io::BufReader::new(child_stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    {
                        let mut s = state.lock().unwrap();
                        s.lines.push(line);
                        if s.lines.len() > MAX_LINES {
                            s.lines.remove(0);
                        }
                    }
                    let _ = update_tx.send(());
                }
            });
        }

        // Read stderr → shared state (prefixed so clients can distinguish)
        {
            let state = state.clone();
            let update_tx = update_tx.clone();
            tokio::spawn(async move {
                use tokio::io::AsyncBufReadExt;
                let mut lines = tokio::io::BufReader::new(child_stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    {
                        let mut s = state.lock().unwrap();
                        s.lines.push(format!("[stderr] {line}"));
                        if s.lines.len() > MAX_LINES {
                            s.lines.remove(0);
                        }
                    }
                    let _ = update_tx.send(());
                }
            });
        }

        // Wait for process exit → update running flag
        {
            let state = state.clone();
            tokio::spawn(async move {
                if let Ok(status) = child.wait().await {
                    let mut s = state.lock().unwrap();
                    s.running = false;
                    s.exit_code = status.code();
                    drop(s);
                    let _ = update_tx.send(());
                }
            });
        }

        let label = if args.is_empty() {
            command.to_string()
        } else {
            format!("{command} {}", args.join(" "))
        };

        Ok(Self { command: label, stdin_tx, state })
    }
}

impl SimpleAuthority for ProcessAuthority {
    type Intent = ProcessIntent;
    type Snapshot = ProcessSnapshot;
    type Passport = ProcessPassport;
    type Error = ProcessError;

    fn on_connect(&mut self, session: &Session) -> Result<(), Self::Error> {
        tracing::info!("{} connected", session.name);
        Ok(())
    }

    fn on_transfer_in(
        &mut self,
        _session: &Session,
        passport: Self::Passport,
    ) -> Result<ImportResult<Self::Passport>, Self::Error> {
        tracing::info!("{} arrived from {}", passport.name, passport.origin);
        Ok(ImportResult::accept(passport))
    }

    fn on_disconnect(&mut self, session: &Session) {
        tracing::info!("{} disconnected", session.name);
    }

    fn handle_intent(
        &mut self,
        session: &Session,
        intent: Self::Intent,
    ) -> Result<(), Self::Error> {
        {
            let s = self.state.lock().unwrap();
            if !s.running {
                return Err(ProcessError::NotRunning);
            }
        }

        match intent {
            ProcessIntent::SendInput { text } => {
                self.stdin_tx
                    .send(text)
                    .map_err(|e| ProcessError::SendFailed(e.to_string()))?;
            }
            ProcessIntent::SendSignal { signal } => {
                // TODO: implement via nix crate; log for now
                tracing::info!(
                    "{} sent {:?} (signal delivery not yet implemented)",
                    session.name,
                    signal
                );
            }
        }
        Ok(())
    }

    fn snapshot(&self) -> Self::Snapshot {
        let s = self.state.lock().unwrap();
        ProcessSnapshot {
            lines: s.lines.clone(),
            running: s.running,
            exit_code: s.exit_code,
            command: self.command.clone(),
        }
    }

    fn emit_passport(&self, session: &Session) -> Self::Passport {
        ProcessPassport {
            name: session.name.clone(),
            origin: self.command.clone(),
        }
    }

    fn validate_destination(&self, _destination: &str) -> bool {
        false
    }
}
