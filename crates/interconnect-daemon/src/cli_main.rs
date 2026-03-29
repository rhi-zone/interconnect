mod cli;
mod config;
mod preset;
mod protocol;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tokio::io::AsyncWriteExt;

use crate::protocol::{Request, Response};

#[derive(Parser)]
#[command(name = "interconnect", about = "CLI client for the Interconnect daemon")]
struct Cli {
    /// Override the daemon socket path (default: ~/.interconnect/daemon.sock).
    /// Can also be set via INTERCONNECT_SOCK.
    #[arg(long, env = "INTERCONNECT_SOCK")]
    socket: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Receive messages from a room (blocks until a message arrives).
    Recv {
        room: String,
        /// Return immediately even if no messages are pending.
        #[arg(long)]
        nowait: bool,
    },
    /// Send an intent JSON payload to a room.
    Send {
        room: String,
        /// JSON payload to send.
        json: String,
    },
    /// Print the current state snapshot for a room.
    State { room: String },
    /// List all configured rooms.
    List,
    /// Generate configuration hooks for a preset.
    Init {
        /// Preset name (currently only "claude").
        #[arg(long)]
        preset: String,
        /// Write output to this file instead of stdout.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Path to interconnect.toml (default: ./interconnect.toml).
        #[arg(long, default_value = "interconnect.toml")]
        config: PathBuf,
    },
    /// Block on a room and invoke a command whenever a message arrives.
    Watch {
        room: String,
        /// Shell command to run for each message. The message JSON is piped to
        /// the command's stdin. INTERCONNECT_REPLY_TO is set to the room name.
        #[arg(long)]
        exec: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let socket_path = cli
        .socket
        .unwrap_or_else(default_socket_path);

    match cli.command {
        Command::Init {
            preset,
            output,
            config: config_path,
        } => {
            handle_init(&preset, output.as_deref(), &config_path)?;
        }

        Command::List => {
            let req = Request::List;
            let resp = cli::send_request(&socket_path, &req).await?;
            print_response(resp);
        }

        Command::State { room } => {
            let req = Request::State { room };
            let resp = cli::send_request(&socket_path, &req).await?;
            print_response(resp);
        }

        Command::Send { room, json } => {
            let payload: serde_json::Value = serde_json::from_str(&json)
                .map_err(|e| anyhow::anyhow!("invalid JSON payload: {e}"))?;
            let req = Request::Send { room, payload };
            let resp = cli::send_request(&socket_path, &req).await?;
            print_response(resp);
        }

        Command::Recv { room, nowait } => {
            let req = Request::Recv {
                room,
                block: !nowait,
            };
            let resp = cli::send_request(&socket_path, &req).await?;
            print_response(resp);
        }

        Command::Watch { room, exec } => {
            handle_watch(&socket_path, &room, &exec).await?;
        }
    }

    Ok(())
}

async fn handle_watch(
    socket_path: &PathBuf,
    room: &str,
    exec: &str,
) -> anyhow::Result<()> {
    loop {
        let req = Request::Recv {
            room: room.to_owned(),
            block: true,
        };
        let resp = cli::send_request(socket_path, &req).await?;

        let messages = match resp {
            Response::Messages { messages, .. } => messages,
            Response::Error { error, .. } => {
                anyhow::bail!("daemon error: {error}");
            }
            other => {
                anyhow::bail!(
                    "unexpected response: {}",
                    serde_json::to_string(&other).unwrap_or_default()
                );
            }
        };

        for msg in messages {
            let msg_json = serde_json::to_string(&msg)?;
            let mut child = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(exec)
                .env("INTERCONNECT_REPLY_TO", room)
                .stdin(std::process::Stdio::piped())
                .spawn()?;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(msg_json.as_bytes()).await;
            }
            child.wait().await?;
        }
    }
}

fn handle_init(
    preset: &str,
    output: Option<&std::path::Path>,
    config_path: &std::path::Path,
) -> anyhow::Result<()> {
    match preset {
        "claude" => {
            let config = if config_path.exists() {
                config::Config::load(config_path)?
            } else {
                eprintln!(
                    "warning: config file not found at {}, generating with no rooms",
                    config_path.display()
                );
                config::Config::default()
            };

            let p = preset::claude::ClaudePreset::from_config(&config);
            let json = serde_json::to_string_pretty(&p.render())?;

            match output {
                None => println!("{json}"),
                Some(path) => {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(path, format!("{json}\n"))?;
                    eprintln!("wrote {}", path.display());
                }
            }
        }
        other => {
            anyhow::bail!("unknown preset: {other} (available: claude)");
        }
    }
    Ok(())
}

/// Pretty-print a daemon response to stdout.
fn print_response(resp: Response) {
    match &resp {
        Response::Messages { messages, .. } => {
            for msg in messages {
                println!("{}", serde_json::to_string_pretty(msg).unwrap());
            }
        }
        Response::Rooms { rooms, .. } => {
            for room in rooms {
                println!("{room}");
            }
        }
        _ => {
            println!("{}", serde_json::to_string_pretty(&resp).unwrap());
        }
    }
}

fn default_socket_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".interconnect").join("daemon.sock")
}
