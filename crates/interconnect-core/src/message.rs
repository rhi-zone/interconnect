//! Protocol messages.
//!
//! The core message types are generic over Intent and Snapshot.
//! Applications define their own types; this crate provides the envelope.

use crate::{Identity, Manifest, Transfer};
use serde::{Deserialize, Serialize};

/// Messages sent from client to server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage<I> {
    /// Authenticate with the server.
    Auth {
        identity: Identity,
        /// Optional passport if transferring from another server.
        passport: Option<Vec<u8>>,
    },
    /// Send an intent (application-defined action request).
    Intent(I),
    /// Acknowledge receipt of a snapshot.
    Ack { seq: u64 },
    /// Request transfer to another server.
    RequestTransfer { destination: String },
}

/// Messages sent from server to client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage<S> {
    /// Server manifest (sent on connection).
    Manifest(Manifest),
    /// Snapshot of current state.
    Snapshot { seq: u64, data: S },
    /// Transfer to another server.
    Transfer(Transfer),
    /// Error or rejection.
    Error { code: String, message: String },
}
