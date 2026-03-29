//! Telegram Bot API transport.
//!
//! Presents a Telegram chat as an Interconnect `Transport`. Long-polling
//! `getUpdates` events become `ServerWire<TelegramSnapshot>` bytes;
//! `ClientWire<TelegramIntent>` bytes become `sendMessage` API calls.

use crate::types::{TelegramError, TelegramIntent, TelegramMessage, TelegramSnapshot};
use interconnect_core::{ClientWire, ServerWire, Transport};
use reqwest::Client;
use std::collections::VecDeque;

pub const MAX_MESSAGES: usize = 50;
/// Long-poll timeout in seconds passed to getUpdates.
const POLL_TIMEOUT_SECS: u64 = 30;

pub struct TelegramTransport {
    pub(crate) http: Client,
    pub(crate) bot_token: String,
    pub(crate) chat_id: i64,
    pub(crate) chat_title: String,
    pub(crate) messages: VecDeque<TelegramMessage>,
    pub(crate) seq: u64,
    /// The `offset` passed to getUpdates (next expected update_id).
    pub(crate) update_offset: i64,
}

impl TelegramTransport {
    pub(crate) fn current_snapshot(&self) -> TelegramSnapshot {
        TelegramSnapshot {
            chat_id: self.chat_id,
            title: self.chat_title.clone(),
            messages: self.messages.iter().cloned().collect(),
        }
    }

    fn push_message(&mut self, msg: TelegramMessage) {
        self.messages.push_back(msg);
        if self.messages.len() > MAX_MESSAGES {
            self.messages.pop_front();
        }
    }

    fn api_url(&self, method: &str) -> String {
        format!(
            "https://api.telegram.org/bot{}/{}",
            self.bot_token, method
        )
    }
}

impl Transport for TelegramTransport {
    type Error = TelegramError;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let wire: ClientWire<TelegramIntent> = serde_json::from_slice(data)?;
        // Auth, Ping, TransferRequest — not applicable for platform connectors.
        if let ClientWire::Intent(TelegramIntent::SendMessage { text }) = wire {
            let body = serde_json::json!({
                "chat_id": self.chat_id,
                "text": text,
            });
            let resp: serde_json::Value = self
                .http
                .post(self.api_url("sendMessage"))
                .json(&body)
                .send()
                .await?
                .json()
                .await?;
            if !resp["ok"].as_bool().unwrap_or(false) {
                let err = resp["description"]
                    .as_str()
                    .unwrap_or("unknown error")
                    .to_string();
                return Err(TelegramError::Api(err));
            }
        }
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<Vec<u8>>, Self::Error> {
        loop {
            let params = serde_json::json!({
                "offset": self.update_offset,
                "timeout": POLL_TIMEOUT_SECS,
                "allowed_updates": ["message"],
            });

            let resp: serde_json::Value = self
                .http
                .post(self.api_url("getUpdates"))
                .json(&params)
                .send()
                .await?
                .json()
                .await?;

            if !resp["ok"].as_bool().unwrap_or(false) {
                let err = resp["description"]
                    .as_str()
                    .unwrap_or("unknown error")
                    .to_string();
                return Err(TelegramError::Api(err));
            }

            let updates = match resp["result"].as_array() {
                Some(arr) => arr.clone(),
                None => continue,
            };

            // No updates yet — poll again.
            if updates.is_empty() {
                continue;
            }

            let mut emitted: Option<Vec<u8>> = None;

            for update in &updates {
                let update_id = update["update_id"].as_i64().unwrap_or(0);
                // Advance offset past this update so we don't see it again.
                if update_id >= self.update_offset {
                    self.update_offset = update_id + 1;
                }

                let message = match update.get("message") {
                    Some(m) => m,
                    None => continue,
                };

                // Only handle messages in our target chat.
                let msg_chat_id = message["chat"]["id"].as_i64().unwrap_or(0);
                if msg_chat_id != self.chat_id {
                    continue;
                }

                // Skip messages without text (photos, stickers, etc.).
                let text = match message["text"].as_str() {
                    Some(t) => t.to_string(),
                    None => continue,
                };

                let message_id = message["message_id"].as_i64().unwrap_or(0) as i32;
                let timestamp = message["date"].as_u64().unwrap_or(0);

                let from = sender_name(message);

                self.push_message(TelegramMessage {
                    message_id,
                    from,
                    text,
                    timestamp,
                });

                let snapshot = self.current_snapshot();
                let wire = ServerWire::<TelegramSnapshot>::Snapshot {
                    seq: self.seq,
                    data: snapshot,
                };
                self.seq += 1;
                // Serialize the last emitted snapshot; deliver after processing all updates.
                emitted = Some(serde_json::to_vec(&wire)?);
            }

            if let Some(bytes) = emitted {
                return Ok(Some(bytes));
            }
            // All updates were for other chats or non-text — poll again.
        }
    }
}

/// Extract the sender's display name from a Telegram message object.
pub(crate) fn sender_name(message: &serde_json::Value) -> String {
    let from = &message["from"];
    if from.is_null() {
        // Channel post — use chat title.
        return message["chat"]["title"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();
    }
    // Prefer first name + optional last name; fall back to username.
    let first = from["first_name"].as_str().unwrap_or("");
    let last = from["last_name"].as_str().unwrap_or("");
    match (first, last) {
        ("", "") => from["username"].as_str().unwrap_or("unknown").to_string(),
        (f, "") => f.to_string(),
        (f, l) => format!("{f} {l}"),
    }
}
