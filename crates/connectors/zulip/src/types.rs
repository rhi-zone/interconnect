//! Zulip-specific protocol types.

use serde::{Deserialize, Serialize};

/// A Zulip message in the room's history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZulipMessage {
    pub id: u64,
    pub sender_email: String,
    pub sender_name: String,
    pub content: String,
    /// Unix timestamp in seconds.
    pub timestamp: u64,
}

/// Snapshot of a Zulip stream/topic room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZulipSnapshot {
    pub realm: String,
    pub stream: String,
    pub topic: String,
    /// Recent messages, oldest first.
    pub messages: Vec<ZulipMessage>,
}

/// Intents a client can send to a Zulip stream/topic room.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ZulipIntent {
    /// Post a message to the stream/topic.
    SendMessage { content: String },
}

/// Errors from Zulip connector operations.
#[derive(Debug, thiserror::Error)]
pub enum ZulipError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("api error: {0}")]
    Api(String),
}

impl From<ZulipError> for interconnect_client::ClientError {
    fn from(e: ZulipError) -> Self {
        interconnect_client::ClientError::Other(Box::new(e))
    }
}
