//! Matrix-specific protocol types.

use serde::{Deserialize, Serialize};

/// A Matrix message in the room's history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixMessage {
    /// Matrix event ID, e.g. "$abc123:server".
    pub event_id: String,
    /// MXID of the sender, e.g. "@user:server".
    pub sender: String,
    /// Plaintext body of the message.
    pub body: String,
    /// Origin server timestamp in milliseconds.
    pub timestamp: u64,
}

/// Snapshot of a Matrix room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixSnapshot {
    pub room_id: String,
    pub room_name: String,
    /// Recent messages, oldest first.
    pub messages: Vec<MatrixMessage>,
}

/// Intents a client can send to a Matrix room.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MatrixIntent {
    /// Post a text message to the room.
    SendMessage { text: String },
}

/// Errors from Matrix connector operations.
#[derive(Debug, thiserror::Error)]
pub enum MatrixError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("matrix api error: {0}")]
    Api(String),
}

impl From<MatrixError> for interconnect_client::ClientError {
    fn from(e: MatrixError) -> Self {
        interconnect_client::ClientError::Other(Box::new(e))
    }
}
