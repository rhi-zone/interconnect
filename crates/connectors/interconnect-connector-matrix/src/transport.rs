//! Matrix Client-Server API transport.
//!
//! Presents a Matrix room as an Interconnect `Transport`. Long-poll sync events
//! become `ServerWire<MatrixSnapshot>` bytes; `ClientWire<MatrixIntent>` bytes
//! become Matrix API calls.

use crate::types::{MatrixError, MatrixIntent, MatrixMessage, MatrixSnapshot};
use interconnect_core::{ClientWire, ServerWire, Transport};
use reqwest::Client;
use std::collections::VecDeque;

pub const MAX_MESSAGES: usize = 50;

/// Long-poll timeout for `/_matrix/client/v3/sync` in milliseconds.
const SYNC_TIMEOUT_MS: u64 = 30_000;

pub struct MatrixTransport {
    pub(crate) http: Client,
    pub(crate) homeserver: String,
    pub(crate) access_token: String,
    pub(crate) room_id: String,
    pub(crate) room_name: String,
    /// The `next_batch` token from the last sync response.
    pub(crate) since: Option<String>,
    pub(crate) messages: VecDeque<MatrixMessage>,
    pub(crate) seq: u64,
    /// Monotonically increasing counter used to generate transaction IDs.
    pub(crate) txn_counter: u64,
}

impl MatrixTransport {
    pub(crate) fn current_snapshot(&self) -> MatrixSnapshot {
        MatrixSnapshot {
            room_id: self.room_id.clone(),
            room_name: self.room_name.clone(),
            messages: self.messages.iter().cloned().collect(),
        }
    }

    fn push_message(&mut self, msg: MatrixMessage) {
        self.messages.push_back(msg);
        if self.messages.len() > MAX_MESSAGES {
            self.messages.pop_front();
        }
    }

    fn next_txn_id(&mut self) -> String {
        let id = self.txn_counter;
        self.txn_counter += 1;
        format!("{id:016x}")
    }

    /// Perform a sync request, returning any new `m.room.message` events from
    /// the target room.  Updates `self.since` on success.
    async fn sync_once(&mut self) -> Result<Vec<MatrixMessage>, MatrixError> {
        // Build a filter that restricts the response to our room.
        let filter = serde_json::json!({
            "room": {
                "rooms": [self.room_id],
                "timeline": { "types": ["m.room.message"] }
            },
            "presence": { "types": [] },
            "account_data": { "types": [] }
        });
        let filter_str = serde_json::to_string(&filter)?;

        let mut req = self
            .http
            .get(format!("{}/_matrix/client/v3/sync", self.homeserver))
            .bearer_auth(&self.access_token)
            .query(&[
                ("filter", filter_str.as_str()),
                ("timeout", &SYNC_TIMEOUT_MS.to_string()),
            ]);

        if let Some(ref since) = self.since {
            req = req.query(&[("since", since.as_str())]);
        }

        let resp: serde_json::Value = req.send().await?.json().await?;

        // Extract next_batch token.
        let next_batch = resp["next_batch"]
            .as_str()
            .ok_or_else(|| MatrixError::Api("missing next_batch in sync response".into()))?
            .to_string();

        // Extract timeline events for our room.
        let events = resp["rooms"]["join"][&self.room_id]["timeline"]["events"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        let mut messages = Vec::new();
        for ev in &events {
            // Only plain text messages.
            if ev["type"].as_str() != Some("m.room.message") {
                continue;
            }
            let content = &ev["content"];
            if content["msgtype"].as_str() != Some("m.text") {
                continue;
            }
            let event_id = ev["event_id"].as_str().unwrap_or("").to_string();
            let sender = ev["sender"].as_str().unwrap_or("").to_string();
            let body = content["body"].as_str().unwrap_or("").to_string();
            let timestamp = ev["origin_server_ts"].as_u64().unwrap_or(0);

            messages.push(MatrixMessage {
                event_id,
                sender,
                body,
                timestamp,
            });
        }

        self.since = Some(next_batch);
        Ok(messages)
    }
}

impl Transport for MatrixTransport {
    type Error = MatrixError;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let wire: ClientWire<MatrixIntent> = serde_json::from_slice(data)?;
        // Auth, Ping, TransferRequest — not applicable for platform connectors.
        if let ClientWire::Intent(MatrixIntent::SendMessage { text }) = wire {
            let txn_id = self.next_txn_id();
            let url = format!(
                "{}/_matrix/client/v3/rooms/{}/send/m.room.message/{}",
                self.homeserver,
                urlencoding_encode(&self.room_id),
                txn_id,
            );
            let body = serde_json::json!({
                "msgtype": "m.text",
                "body": text,
            });
            let resp: serde_json::Value = self
                .http
                .put(&url)
                .bearer_auth(&self.access_token)
                .json(&body)
                .send()
                .await?
                .json()
                .await?;

            // A successful send returns `{"event_id": "..."}`.
            if resp.get("event_id").is_none() {
                let err = resp["error"].as_str().unwrap_or("unknown send error").to_string();
                return Err(MatrixError::Api(err));
            }
        }
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<Vec<u8>>, Self::Error> {
        loop {
            let new_messages = self.sync_once().await?;

            if new_messages.is_empty() {
                // Long poll returned no new messages — loop immediately.
                continue;
            }

            for msg in new_messages {
                self.push_message(msg);
            }

            let snapshot = self.current_snapshot();
            let wire = ServerWire::<MatrixSnapshot>::Snapshot {
                seq: self.seq,
                data: snapshot,
            };
            self.seq += 1;
            return Ok(Some(serde_json::to_vec(&wire)?));
        }
    }
}

/// Percent-encode a string for use in a URL path segment.
///
/// Matrix room IDs contain `!` and `:` which must be encoded when embedded in
/// a URL path.  We only need a minimal encoder here — the standard library
/// doesn't provide one and pulling in the `percent-encoding` crate just for
/// this is unnecessary.
fn urlencoding_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b => {
                out.push('%');
                out.push(char::from_digit((b >> 4) as u32, 16).unwrap());
                out.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
            }
        }
    }
    out
}
