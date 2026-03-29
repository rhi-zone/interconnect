//! Slack-specific protocol types.

use serde::{Deserialize, Serialize};

/// A Slack message in the room's history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackMessage {
    /// Slack timestamp string (unique message ID), e.g. "1234567890.123456".
    pub ts: String,
    pub user_id: String,
    pub user_name: String,
    pub text: String,
    /// Unix timestamp in seconds, derived from ts.
    pub timestamp: u64,
}

/// Snapshot of a Slack channel room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackSnapshot {
    pub channel_id: String,
    pub channel_name: String,
    /// Recent messages, oldest first.
    pub messages: Vec<SlackMessage>,
}

/// Intents a client can send to a Slack channel room.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SlackIntent {
    /// Post a message to the channel.
    SendMessage { text: String },
}

/// Errors from Slack connector operations.
#[derive(Debug, thiserror::Error)]
pub enum SlackError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("websocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("slack api error: {0}")]
    Api(String),

    #[error("connection closed")]
    Closed,
}

impl From<SlackError> for interconnect_client::ClientError {
    fn from(e: SlackError) -> Self {
        interconnect_client::ClientError::Other(Box::new(e))
    }
}
