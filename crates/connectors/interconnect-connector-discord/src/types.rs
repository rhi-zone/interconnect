//! Discord-specific protocol types.

use serde::{Deserialize, Serialize};

/// A Discord message in the room's history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordMessage {
    /// Snowflake ID as string.
    pub id: String,
    pub author_id: String,
    pub author_name: String,
    pub content: String,
    /// Unix timestamp in seconds, derived from snowflake.
    pub timestamp: u64,
}

/// Snapshot of a Discord channel room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordSnapshot {
    pub channel_id: String,
    pub channel_name: String,
    /// Recent messages, oldest first.
    pub messages: Vec<DiscordMessage>,
}

/// Intents a client can send to a Discord channel room.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DiscordIntent {
    /// Post a message to the channel.
    SendMessage { content: String },
    /// Add a reaction to a message.
    React { message_id: String, emoji: String },
}

/// Errors from Discord connector operations.
#[derive(Debug, thiserror::Error)]
pub enum DiscordError {
    #[error("gateway error: {0}")]
    Gateway(#[from] twilight_gateway::error::ReceiveMessageError),

    #[error("http error: {0}")]
    Http(#[from] twilight_http::error::Error),

    #[error("response body error: {0}")]
    Body(#[from] twilight_http::response::DeserializeBodyError),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<DiscordError> for interconnect_client::ClientError {
    fn from(e: DiscordError) -> Self {
        interconnect_client::ClientError::Other(Box::new(e))
    }
}
