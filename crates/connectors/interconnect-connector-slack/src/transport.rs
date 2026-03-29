//! Slack Socket Mode transport.
//!
//! Presents a Slack channel as an Interconnect `Transport`. Socket Mode events
//! become `ServerWire<SlackSnapshot>` bytes; `ClientWire<SlackIntent>` bytes
//! become Web API calls.

use crate::types::{SlackError, SlackIntent, SlackMessage, SlackSnapshot};
use futures_util::{SinkExt, StreamExt};
use interconnect_core::{ClientWire, ServerWire, Transport};
use reqwest::Client;
use serde::Deserialize;
use std::collections::VecDeque;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message};

pub const MAX_MESSAGES: usize = 50;

pub struct SlackTransport {
    pub(crate) ws: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    pub(crate) http: Client,
    pub(crate) bot_token: String,
    pub(crate) channel_id: String,
    pub(crate) channel_name: String,
    pub(crate) messages: VecDeque<SlackMessage>,
    pub(crate) seq: u64,
}

/// Partial structure of a Slack Socket Mode envelope.
#[derive(Debug, Deserialize)]
struct SocketEnvelope {
    envelope_id: Option<String>,
    #[serde(rename = "type")]
    event_type: String,
    payload: Option<serde_json::Value>,
}

/// Partial structure of a Slack events_api payload.
#[derive(Debug, Deserialize)]
struct EventPayload {
    event: Option<serde_json::Value>,
}

impl SlackTransport {
    pub(crate) fn current_snapshot(&self) -> SlackSnapshot {
        SlackSnapshot {
            channel_id: self.channel_id.clone(),
            channel_name: self.channel_name.clone(),
            messages: self.messages.iter().cloned().collect(),
        }
    }

    fn push_message(&mut self, msg: SlackMessage) {
        self.messages.push_back(msg);
        if self.messages.len() > MAX_MESSAGES {
            self.messages.pop_front();
        }
    }

    /// Parse a Slack ts string into Unix seconds.
    pub(crate) fn parse_ts(ts: &str) -> u64 {
        ts.split('.').next().unwrap_or("0").parse::<u64>().unwrap_or(0)
    }

    /// Send an acknowledgement for a Socket Mode envelope.
    async fn ack(&mut self, envelope_id: &str) -> Result<(), SlackError> {
        let ack = serde_json::json!({
            "envelope_id": envelope_id,
        });
        self.ws.send(Message::Text(ack.to_string().into())).await?;
        Ok(())
    }
}

impl Transport for SlackTransport {
    type Error = SlackError;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let wire: ClientWire<SlackIntent> = serde_json::from_slice(data)?;
        // Auth, Ping, TransferRequest — not applicable for platform connectors.
        if let ClientWire::Intent(SlackIntent::SendMessage { text }) = wire {
            let body = serde_json::json!({
                "channel": self.channel_id,
                "text": text,
            });
            let resp: serde_json::Value = self
                .http
                .post("https://slack.com/api/chat.postMessage")
                .bearer_auth(&self.bot_token)
                .json(&body)
                .send()
                .await?
                .json()
                .await?;
            if !resp["ok"].as_bool().unwrap_or(false) {
                let err = resp["error"].as_str().unwrap_or("unknown").to_string();
                return Err(SlackError::Api(err));
            }
        }
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<Vec<u8>>, Self::Error> {
        loop {
            let msg = match self.ws.next().await {
                Some(Ok(m)) => m,
                Some(Err(e)) => return Err(SlackError::WebSocket(e)),
                None => return Ok(None),
            };

            let text = match msg {
                Message::Text(t) => t,
                Message::Close(_) => return Ok(None),
                // Ping/Pong/Binary — ignore
                _ => continue,
            };

            let envelope: SocketEnvelope = match serde_json::from_str(&text) {
                Ok(e) => e,
                Err(_) => continue,
            };

            // Acknowledge the envelope if it has an ID.
            if let Some(ref eid) = envelope.envelope_id.clone() {
                self.ack(eid).await?;
            }

            // Handle hello (initial connection confirmation) — no snapshot needed.
            if envelope.event_type == "hello" {
                continue;
            }

            // Handle events_api envelopes.
            if envelope.event_type == "events_api" {
                let payload = match envelope.payload {
                    Some(p) => p,
                    None => continue,
                };

                let ep: EventPayload = match serde_json::from_value(payload) {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                let event = match ep.event {
                    Some(e) => e,
                    None => continue,
                };

                let event_type = event["type"].as_str().unwrap_or("");
                if event_type != "message" {
                    continue;
                }

                // Only handle messages in our target channel.
                let channel = event["channel"].as_str().unwrap_or("");
                if channel != self.channel_id {
                    continue;
                }

                // Skip subtypes (edits, deletions, joins) — only plain messages.
                if event.get("subtype").is_some() {
                    continue;
                }

                let ts = event["ts"].as_str().unwrap_or("0").to_string();
                let user_id = event["user"].as_str().unwrap_or("").to_string();
                let text = event["text"].as_str().unwrap_or("").to_string();
                let timestamp = Self::parse_ts(&ts);

                // Fetch display name via users.info when possible; fall back to user_id.
                let user_name = if !user_id.is_empty() {
                    self.fetch_user_name(&user_id).await.unwrap_or_else(|_| user_id.clone())
                } else {
                    user_id.clone()
                };

                self.push_message(SlackMessage {
                    ts,
                    user_id,
                    user_name,
                    text,
                    timestamp,
                });

                let snapshot = self.current_snapshot();
                let wire = ServerWire::<SlackSnapshot>::Snapshot {
                    seq: self.seq,
                    data: snapshot,
                };
                self.seq += 1;
                return Ok(Some(serde_json::to_vec(&wire)?));
            }
        }
    }
}

impl SlackTransport {
    /// Fetch the display name for a user from the Slack Web API.
    pub(crate) async fn fetch_user_name(&self, user_id: &str) -> Result<String, SlackError> {
        let resp: serde_json::Value = self
            .http
            .get("https://slack.com/api/users.info")
            .bearer_auth(&self.bot_token)
            .query(&[("user", user_id)])
            .send()
            .await?
            .json()
            .await?;

        if !resp["ok"].as_bool().unwrap_or(false) {
            return Err(SlackError::Api(
                resp["error"].as_str().unwrap_or("unknown").to_string(),
            ));
        }

        let name = resp["user"]["profile"]["display_name"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| resp["user"]["real_name"].as_str())
            .unwrap_or(user_id)
            .to_string();

        Ok(name)
    }
}
