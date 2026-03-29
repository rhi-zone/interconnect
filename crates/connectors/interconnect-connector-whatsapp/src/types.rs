//! WhatsApp-specific protocol types.

use serde::{Deserialize, Serialize};

/// A WhatsApp message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppMessage {
    /// Message ID assigned by the WhatsApp Cloud API.
    pub id: String,
    /// Phone number the message was sent from (E.164 format, e.g. "15551234567").
    pub from: String,
    /// Message text content.
    pub text: String,
    /// Unix timestamp in seconds.
    pub timestamp: u64,
}

/// Snapshot of a WhatsApp conversation room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppSnapshot {
    /// The sending phone number ID (Meta Business API identifier, not the phone number itself).
    pub phone_number_id: String,
    /// Recipient phone number in E.164 format (e.g. "15551234567").
    pub recipient: String,
    /// Messages known locally, oldest first.
    ///
    /// The Cloud API is webhook-based for receiving; this list is populated
    /// only when webhook delivery has been wired up externally.
    pub messages: Vec<WhatsAppMessage>,
}

/// Intents a client can send to a WhatsApp conversation room.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WhatsAppIntent {
    /// Send a text message to the recipient.
    SendMessage { text: String },
}

/// Errors from WhatsApp connector operations.
#[derive(Debug, thiserror::Error)]
pub enum WhatsAppError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("whatsapp api error: {0}")]
    Api(String),
}

impl From<WhatsAppError> for interconnect_client::ClientError {
    fn from(e: WhatsAppError) -> Self {
        interconnect_client::ClientError::Other(Box::new(e))
    }
}
