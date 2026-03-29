//! IRC TCP transport.
//!
//! Presents an IRC channel as an Interconnect `Transport`. Incoming PRIVMSGs
//! become `ServerWire<IrcSnapshot>` bytes; `ClientWire<IrcIntent>` bytes
//! become IRC PRIVMSG commands. PING messages are handled automatically and
//! never surfaced as snapshots.

use crate::types::{IrcError, IrcIntent, IrcMessage, IrcSnapshot};
use interconnect_core::{ClientWire, ServerWire, Transport};
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncWriteExt, BufReader, Lines};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

pub const MAX_MESSAGES: usize = 50;

pub struct IrcTransport {
    pub(crate) lines: Lines<BufReader<OwnedReadHalf>>,
    pub(crate) writer: OwnedWriteHalf,
    pub(crate) channel: String,
    pub(crate) server: String,
    pub(crate) messages: VecDeque<IrcMessage>,
    pub(crate) seq: u64,
}

impl IrcTransport {
    pub(crate) fn current_snapshot(&self) -> IrcSnapshot {
        IrcSnapshot {
            channel: self.channel.clone(),
            server: self.server.clone(),
            messages: self.messages.iter().cloned().collect(),
        }
    }

    fn push_message(&mut self, msg: IrcMessage) {
        self.messages.push_back(msg);
        if self.messages.len() > MAX_MESSAGES {
            self.messages.pop_front();
        }
    }

    async fn send_raw(&mut self, line: &str) -> Result<(), IrcError> {
        self.writer
            .write_all(format!("{line}\r\n").as_bytes())
            .await?;
        Ok(())
    }
}

/// Parse the nick out of a prefix like "nick!user@host".
fn parse_nick(prefix: &str) -> &str {
    prefix.split('!').next().unwrap_or(prefix)
}

/// Current Unix timestamp in seconds.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl Transport for IrcTransport {
    type Error = IrcError;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let wire: ClientWire<IrcIntent> = serde_json::from_slice(data)?;
        if let ClientWire::Intent(IrcIntent::SendMessage { text }) = wire {
            self.send_raw(&format!("PRIVMSG {} :{}", self.channel, text))
                .await?;
        }
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<Vec<u8>>, Self::Error> {
        loop {
            let line = match self.lines.next_line().await? {
                Some(l) => l,
                None => return Ok(None),
            };

            // Strip optional leading ":"
            // IRC line format: [':' prefix SP] command [params] [':' trailing]
            let line = line.trim_end_matches('\r');

            // Handle PING — reply and continue without emitting a snapshot.
            if let Some(rest) = line.strip_prefix("PING ") {
                self.send_raw(&format!("PONG {rest}")).await?;
                continue;
            }

            // Parse prefix, command, params.
            let (prefix, rest) = if let Some(stripped) = line.strip_prefix(':') {
                let mut parts = stripped.splitn(2, ' ');
                let pfx = parts.next().unwrap_or("");
                let rest = parts.next().unwrap_or("");
                (Some(pfx), rest)
            } else {
                (None, line)
            };

            let mut parts = rest.splitn(2, ' ');
            let command = parts.next().unwrap_or("");
            let params = parts.next().unwrap_or("");

            match command {
                "PRIVMSG" => {
                    // params: "<target> :<text>"
                    let mut p = params.splitn(2, " :");
                    let target = p.next().unwrap_or("").trim();
                    let text = p.next().unwrap_or("");

                    // Only handle messages directed to our channel.
                    if !target.eq_ignore_ascii_case(&self.channel) {
                        continue;
                    }

                    let nick = prefix.map(parse_nick).unwrap_or("").to_string();
                    let timestamp = now_secs();

                    self.push_message(IrcMessage {
                        nick,
                        text: text.to_string(),
                        timestamp,
                    });

                    let snapshot = self.current_snapshot();
                    let wire = ServerWire::<IrcSnapshot>::Snapshot {
                        seq: self.seq,
                        data: snapshot,
                    };
                    self.seq += 1;
                    return Ok(Some(serde_json::to_vec(&wire)?));
                }
                // Ignore all other commands (MODE, JOIN, PART, NOTICE, numeric replies, etc.)
                _ => continue,
            }
        }
    }
}
