//! Chat-specific protocol types.

use interconnect_core::Identity;
use serde::{Deserialize, Serialize};

/// Chat intents (what clients can request).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatIntent {
    /// Send a message to the room.
    Message { text: String },
    /// Request transfer to another server.
    Transfer { destination: String },
}

/// Chat snapshot (current room state).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSnapshot {
    /// Recent messages (newest last).
    pub messages: Vec<ChatMessage>,
    /// Users currently in the room.
    pub users: Vec<String>,
}

/// A chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub from: String,
    pub text: String,
    pub timestamp: u64,
}

/// Chat passport (what transfers between servers).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatPassport {
    /// Display name.
    pub name: String,
    /// Where they came from.
    pub origin: String,
}

impl ChatPassport {
    pub fn new(name: String, origin: String) -> Self {
        Self { name, origin }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        serde_json::from_slice(data).ok()
    }
}

/// Wrapper for messages over the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WireMessage {
    // Client -> Server
    Auth {
        identity: Identity,
        passport: Option<Vec<u8>>,
    },
    Intent(ChatIntent),

    // Server -> Client
    Manifest {
        name: String,
        identity: Identity,
    },
    Snapshot(ChatSnapshot),
    Transfer {
        destination: String,
        passport: Vec<u8>,
    },
    Error {
        message: String,
    },
    /// System message (user joined, left, etc.)
    System {
        text: String,
    },
}
