//! Signal-specific protocol types.

use serde::{Deserialize, Serialize};

/// A Signal message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalMessage {
    /// Sender phone number or group ID.
    pub sender: String,
    /// Message body text.
    pub text: String,
    /// Unix timestamp in milliseconds (as provided by Signal).
    pub timestamp: u64,
}

/// Snapshot of a Signal conversation room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalSnapshot {
    /// The local Signal account (registered phone number).
    pub account: String,
    /// The conversation partner — phone number for 1:1, group ID for groups.
    pub recipient: String,
    /// Recent messages, oldest first (up to 50).
    pub messages: Vec<SignalMessage>,
}

/// Intents a client can send to a Signal conversation room.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalIntent {
    /// Send a text message to the conversation.
    SendMessage { text: String },
}

/// Errors from Signal connector operations.
#[derive(Debug, thiserror::Error)]
pub enum SignalError {
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("signal-cli process error: {0}")]
    Process(String),

    #[error("connection closed")]
    Closed,
}

impl From<SignalError> for interconnect_client::ClientError {
    fn from(e: SignalError) -> Self {
        interconnect_client::ClientError::Other(Box::new(e))
    }
}
