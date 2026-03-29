//! Telegram-specific protocol types.

use serde::{Deserialize, Serialize};

/// A Telegram message in the room's history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramMessage {
    /// Telegram message ID.
    pub message_id: i32,
    /// Display name of the sender (first name, or username as fallback).
    pub from: String,
    pub text: String,
    /// Unix timestamp in seconds.
    pub timestamp: u64,
}

/// Snapshot of a Telegram chat room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramSnapshot {
    pub chat_id: i64,
    /// Chat title (group/channel name, or display name for private chats).
    pub title: String,
    /// Recent messages, oldest first (up to 50).
    pub messages: Vec<TelegramMessage>,
}

/// Intents a client can send to a Telegram chat room.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TelegramIntent {
    /// Send a text message to the chat.
    SendMessage { text: String },
}

/// Errors from the Telegram connector.
#[derive(Debug, thiserror::Error)]
pub enum TelegramError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("telegram api error: {0}")]
    Api(String),

    #[error("connection closed")]
    Closed,
}

impl From<TelegramError> for interconnect_client::ClientError {
    fn from(e: TelegramError) -> Self {
        interconnect_client::ClientError::Other(Box::new(e))
    }
}
