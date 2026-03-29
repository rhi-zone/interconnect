//! Signal transport via signal-cli JSON-RPC over stdio.
//!
//! Presents a Signal conversation as an Interconnect `Transport`. Incoming
//! JSON-RPC notifications from signal-cli's stdout become
//! `ServerWire<SignalSnapshot>` bytes; `ClientWire<SignalIntent>` bytes become
//! JSON-RPC `send` requests written to signal-cli's stdin.

use crate::types::{SignalError, SignalIntent, SignalMessage, SignalSnapshot};
use interconnect_core::{ClientWire, ServerWire, Transport};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncWriteExt, BufReader, Lines};
use tokio::process::{ChildStdin, ChildStdout};

pub const MAX_MESSAGES: usize = 50;

/// Counter for JSON-RPC request IDs (process-global is fine; each connection
/// has its own subprocess, but a shared counter avoids any accidental reuse).
static RPC_ID: AtomicU64 = AtomicU64::new(1);

pub struct SignalTransport {
    pub(crate) stdin: ChildStdin,
    pub(crate) stdout: Lines<BufReader<ChildStdout>>,
    pub(crate) account: String,
    pub(crate) recipient: String,
    pub(crate) messages: VecDeque<SignalMessage>,
    pub(crate) seq: u64,
}

impl SignalTransport {
    pub(crate) fn current_snapshot(&self) -> SignalSnapshot {
        SignalSnapshot {
            account: self.account.clone(),
            recipient: self.recipient.clone(),
            messages: self.messages.iter().cloned().collect(),
        }
    }

    fn push_message(&mut self, msg: SignalMessage) {
        self.messages.push_back(msg);
        if self.messages.len() > MAX_MESSAGES {
            self.messages.pop_front();
        }
    }

    /// Write a JSON-RPC request line to signal-cli's stdin.
    async fn write_rpc(&mut self, method: &str, params: serde_json::Value) -> Result<(), SignalError> {
        let id = RPC_ID.fetch_add(1, Ordering::Relaxed);
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": id,
        });
        let mut line = serde_json::to_string(&request)?;
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).await?;
        Ok(())
    }
}

impl Transport for SignalTransport {
    type Error = SignalError;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let wire: ClientWire<SignalIntent> = serde_json::from_slice(data)?;
        // Auth, Ping, TransferRequest — not applicable for platform connectors.
        if let ClientWire::Intent(SignalIntent::SendMessage { text }) = wire {
            let params = serde_json::json!({
                "recipient": [self.recipient.clone()],
                "message": text,
                "account": self.account.clone(),
            });
            self.write_rpc("send", params).await?;
        }
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<Vec<u8>>, Self::Error> {
        loop {
            let line = match self.stdout.next_line().await? {
                Some(l) => l,
                None => return Ok(None),
            };

            let value: serde_json::Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            // We only care about JSON-RPC notifications (no "id" field, has "method").
            if value.get("id").is_some() {
                // This is a response to one of our requests — ignore.
                continue;
            }

            let method = match value["method"].as_str() {
                Some(m) => m,
                None => continue,
            };

            if method != "receive" {
                continue;
            }

            let envelope = &value["params"]["envelope"];

            // Determine the sender.
            let sender = envelope["sourceNumber"]
                .as_str()
                .or_else(|| envelope["sourceUuid"].as_str())
                .unwrap_or("")
                .to_string();

            // Filter: only messages from our conversation partner (or from ourselves
            // in a group where recipient is the group ID).
            let is_group = self.recipient.starts_with("group.");
            if !is_group && sender != self.recipient {
                continue;
            }

            // For group messages, check the groupId matches.
            if is_group {
                let group_id = envelope["dataMessage"]["groupInfo"]["groupId"]
                    .as_str()
                    .unwrap_or("");
                // recipient is stored as "group.<base64id>"; signal-cli reports raw base64.
                let expected = self.recipient.trim_start_matches("group.");
                if group_id != expected {
                    continue;
                }
            }

            let text = envelope["dataMessage"]["message"]
                .as_str()
                .unwrap_or("")
                .to_string();

            let timestamp = envelope["timestamp"].as_u64().unwrap_or(0);

            self.push_message(SignalMessage {
                sender,
                text,
                timestamp,
            });

            let snapshot = self.current_snapshot();
            let wire = ServerWire::<SignalSnapshot>::Snapshot {
                seq: self.seq,
                data: snapshot,
            };
            self.seq += 1;
            return Ok(Some(serde_json::to_vec(&wire)?));
        }
    }
}
