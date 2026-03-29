//! Zulip HTTP long-poll transport.
//!
//! Presents a Zulip stream/topic as an Interconnect `Transport`. Long-poll
//! events become `ServerWire<ZulipSnapshot>` bytes; `ClientWire<ZulipIntent>`
//! bytes become Zulip HTTP API calls.

use crate::types::{ZulipError, ZulipIntent, ZulipMessage, ZulipSnapshot};
use interconnect_core::{ClientWire, ServerWire, Transport};
use reqwest::Client;
use serde::Deserialize;
use std::collections::VecDeque;

pub const MAX_MESSAGES: usize = 50;

pub struct ZulipTransport {
    pub(crate) client: Client,
    pub(crate) realm: String,
    pub(crate) email: String,
    pub(crate) api_key: String,
    pub(crate) stream: String,
    pub(crate) topic: String,
    pub(crate) queue_id: String,
    pub(crate) last_event_id: i64,
    pub(crate) messages: VecDeque<ZulipMessage>,
    pub(crate) seq: u64,
}

// --- Zulip API response types ---

#[derive(Debug, Deserialize)]
struct ZulipApiMessage {
    id: u64,
    sender_email: String,
    sender_full_name: String,
    content: String,
    timestamp: u64,
    #[serde(rename = "display_recipient")]
    stream_name: Option<serde_json::Value>,
    subject: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    messages: Vec<ZulipApiMessage>,
}

#[derive(Debug, Deserialize)]
struct RegisterResponse {
    queue_id: String,
    last_event_id: i64,
}

#[derive(Debug, Deserialize)]
struct EventMessage {
    id: i64,
    #[serde(rename = "type")]
    event_type: String,
    message: Option<ZulipApiMessage>,
}

#[derive(Debug, Deserialize)]
struct EventsResponse {
    events: Vec<EventMessage>,
}

impl ZulipTransport {
    pub(crate) fn current_snapshot(&self) -> ZulipSnapshot {
        ZulipSnapshot {
            realm: self.realm.clone(),
            stream: self.stream.clone(),
            topic: self.topic.clone(),
            messages: self.messages.iter().cloned().collect(),
        }
    }

    fn push_message(&mut self, msg: ZulipMessage) {
        self.messages.push_back(msg);
        if self.messages.len() > MAX_MESSAGES {
            self.messages.pop_front();
        }
    }

    fn message_matches(&self, api_msg: &ZulipApiMessage) -> bool {
        // Check topic matches (Zulip calls it "subject").
        let topic_matches = api_msg
            .subject
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case(&self.topic))
            .unwrap_or(false);

        // Check stream matches. display_recipient is the stream name for stream messages.
        let stream_matches = api_msg
            .stream_name
            .as_ref()
            .and_then(|v| v.as_str())
            .map(|s| s.eq_ignore_ascii_case(&self.stream))
            .unwrap_or(false);

        topic_matches && stream_matches
    }

    fn convert_message(api_msg: ZulipApiMessage) -> ZulipMessage {
        ZulipMessage {
            id: api_msg.id,
            sender_email: api_msg.sender_email,
            sender_name: api_msg.sender_full_name,
            content: api_msg.content,
            timestamp: api_msg.timestamp,
        }
    }

    /// Fetch recent messages via HTTP and populate the internal buffer.
    pub(crate) async fn fetch_initial_snapshot(&mut self) -> Result<ZulipSnapshot, ZulipError> {
        let narrow = serde_json::to_string(&serde_json::json!([
            {"operator": "stream", "operand": self.stream},
            {"operator": "topic", "operand": self.topic}
        ]))?;

        let resp = self
            .client
            .get(format!("{}/api/v1/messages", self.realm))
            .basic_auth(&self.email, Some(&self.api_key))
            .query(&[
                ("anchor", "newest"),
                ("num_before", "50"),
                ("num_after", "0"),
                ("narrow", &narrow),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<MessagesResponse>()
            .await?;

        // API returns oldest-first; push in order.
        for api_msg in resp.messages {
            self.messages.push_back(Self::convert_message(api_msg));
        }

        Ok(self.current_snapshot())
    }
}

impl Transport for ZulipTransport {
    type Error = ZulipError;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let wire: ClientWire<ZulipIntent> = serde_json::from_slice(data)?;
        // Auth, Ping, TransferRequest — not applicable for platform connectors.
        if let ClientWire::Intent(ZulipIntent::SendMessage { content }) = wire {
            self.client
                .post(format!("{}/api/v1/messages", self.realm))
                .basic_auth(&self.email, Some(&self.api_key))
                .form(&[
                    ("type", "stream"),
                    ("to", &self.stream),
                    ("topic", &self.topic),
                    ("content", &content),
                ])
                .send()
                .await?
                .error_for_status()?;
        }
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<Vec<u8>>, Self::Error> {
        loop {
            let resp = self
                .client
                .get(format!("{}/api/v1/events", self.realm))
                .basic_auth(&self.email, Some(&self.api_key))
                .query(&[
                    ("queue_id", self.queue_id.as_str()),
                    ("last_event_id", &self.last_event_id.to_string()),
                    ("dont_block", "false"),
                ])
                .send()
                .await?
                .error_for_status()?
                .json::<EventsResponse>()
                .await?;

            let mut new_message: Option<ZulipMessage> = None;

            for event in resp.events {
                // Always advance last_event_id.
                if event.id > self.last_event_id {
                    self.last_event_id = event.id;
                }

                if event.event_type == "message"
                    && let Some(api_msg) = event.message
                    && self.message_matches(&api_msg)
                {
                    new_message = Some(Self::convert_message(api_msg));
                }
            }

            if let Some(msg) = new_message {
                self.push_message(msg);
                let snapshot = self.current_snapshot();
                let wire = ServerWire::<ZulipSnapshot>::Snapshot {
                    seq: self.seq,
                    data: snapshot,
                };
                self.seq += 1;
                return Ok(Some(serde_json::to_vec(&wire)?));
            }

            // No matching message in this poll batch — loop and poll again.
        }
    }
}

/// Register an event queue with the Zulip server.
pub async fn register_event_queue(
    client: &Client,
    realm: &str,
    email: &str,
    api_key: &str,
) -> Result<(String, i64), ZulipError> {
    let resp = client
        .post(format!("{realm}/api/v1/register"))
        .basic_auth(email, Some(api_key))
        .form(&[("event_types", r#"["message"]"#)])
        .send()
        .await?
        .error_for_status()?
        .json::<RegisterResponse>()
        .await?;

    Ok((resp.queue_id, resp.last_event_id))
}
