//! Mailing list protocol types.

use serde::{Deserialize, Serialize};

/// A single campaign message in the list archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailMessage {
    pub id: u32,
    pub subject: String,
    /// Plain text or HTML body (from campaign body field).
    pub body: String,
    /// Unix timestamp in seconds.
    pub sent_at: u64,
}

/// Snapshot of a mailing list room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailSnapshot {
    pub list_id: u32,
    pub list_name: String,
    pub base_url: String,
    /// Recent campaigns, oldest first, up to 50 entries.
    pub messages: Vec<MailMessage>,
}

/// Intents a client can send to a mailing list room.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MailIntent {
    /// Create and send a campaign to the list.
    SendMessage { subject: String, body: String },
}

/// Errors from mailing list connector operations.
#[derive(Debug, thiserror::Error)]
pub enum MailError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("api error: {0}")]
    Api(String),
}

impl From<MailError> for interconnect_client::ClientError {
    fn from(e: MailError) -> Self {
        interconnect_client::ClientError::Other(Box::new(e))
    }
}
