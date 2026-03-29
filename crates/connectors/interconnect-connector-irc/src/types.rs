//! IRC-specific protocol types.

use serde::{Deserialize, Serialize};

/// A single IRC message received in the channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrcMessage {
    /// Nick of the sender.
    pub nick: String,
    /// Message text.
    pub text: String,
    /// Unix timestamp (seconds) when the message was received.
    pub timestamp: u64,
}

/// Snapshot of an IRC channel room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrcSnapshot {
    /// IRC channel name, e.g. "#rust".
    pub channel: String,
    /// IRC server hostname.
    pub server: String,
    /// Recent messages, oldest first (up to 50).
    pub messages: Vec<IrcMessage>,
}

/// Intents a client can send to an IRC channel room.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IrcIntent {
    /// Post a message to the channel.
    SendMessage { text: String },
}

/// Errors from IRC connector operations.
#[derive(Debug, thiserror::Error)]
pub enum IrcError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("irc error: {0}")]
    Protocol(String),

    #[error("connection closed")]
    Closed,
}

impl From<IrcError> for interconnect_client::ClientError {
    fn from(e: IrcError) -> Self {
        interconnect_client::ClientError::Other(Box::new(e))
    }
}
