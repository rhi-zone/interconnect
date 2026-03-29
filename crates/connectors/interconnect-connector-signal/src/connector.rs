//! High-level connector entry point.

use crate::transport::SignalTransport;
use crate::types::{SignalError, SignalIntent, SignalSnapshot};
use interconnect_client::Connection;
use interconnect_core::{Identity, Manifest};
use std::collections::VecDeque;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub type SignalConnection = Connection<SignalTransport, SignalIntent, SignalSnapshot>;

/// Connect to a Signal conversation as an Interconnect room.
///
/// # Requirements
///
/// - `signal_cli_path` must point to a working `signal-cli` installation.
/// - `account` must be a phone number already registered with signal-cli
///   (run `signal-cli -a <number> register` and `signal-cli -a <number> verify <code>`
///   beforehand).
/// - End-to-end encryption is handled transparently by signal-cli; this
///   connector never sees plaintext keys.
///
/// # Group chats
///
/// Pass the group ID (as reported by `signal-cli listGroups`) as `recipient`.
/// Prefix it with `"group."` — e.g. `"group.abc123=="`. The connector filters
/// incoming envelopes by group ID and sends to the group automatically.
///
/// # Example
///
/// ```ignore
/// use interconnect_connector_signal as signal;
///
/// let (mut conn, snapshot) = signal::connect(
///     "/usr/bin/signal-cli",
///     "+15550001234",
///     "+15559876543",
/// ).await?;
///
/// for msg in &snapshot.messages {
///     println!("{}: {}", msg.sender, msg.text);
/// }
///
/// conn.send_intent(signal::SignalIntent::SendMessage {
///     text: "hello from another room".to_string(),
/// }).await?;
/// ```
pub async fn connect(
    signal_cli_path: impl AsRef<Path>,
    account: impl Into<String>,
    recipient: impl Into<String>,
) -> Result<(SignalConnection, SignalSnapshot), SignalError> {
    let account = account.into();
    let recipient = recipient.into();

    let mut child = Command::new(signal_cli_path.as_ref())
        .args([
            "--output=json",
            "-a",
            &account,
            "jsonRpc",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(SignalError::Io)?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| SignalError::Process("failed to open stdin for signal-cli".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| SignalError::Process("failed to open stdout for signal-cli".into()))?;

    let stdout_lines = BufReader::new(stdout).lines();

    let transport = SignalTransport {
        stdin,
        stdout: stdout_lines,
        account: account.clone(),
        recipient: recipient.clone(),
        messages: VecDeque::new(),
        seq: 0,
    };

    let initial_snapshot = transport.current_snapshot();

    let manifest = Manifest {
        identity: Identity::local(format!("signal:{account}:{recipient}")),
        name: recipient.clone(),
        substrate: None,
        metadata: serde_json::json!({
            "type": "signal",
            "account": account,
            "recipient": recipient,
        }),
    };

    let conn = SignalConnection::established(transport, manifest);
    Ok((conn, initial_snapshot))
}
