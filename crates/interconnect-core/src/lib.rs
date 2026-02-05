//! Core types and traits for Interconnect.
//!
//! This crate provides the protocol primitives. Applications define their own
//! Intent, Snapshot, and Passport types; this crate provides the framing.

mod identity;
mod message;
mod transfer;

pub use identity::Identity;
pub use message::{ClientMessage, ServerMessage};
pub use transfer::{Passport, Transfer};

use serde::{Deserialize, Serialize};

/// Manifest describing a server's capabilities and requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Server's identity (for verification).
    pub identity: Identity,
    /// Human-readable server name.
    pub name: String,
    /// Substrate hash (if applicable).
    pub substrate: Option<String>,
    /// Additional metadata (app-defined).
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Connection lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Establishing connection.
    Connecting,
    /// Receiving initial state.
    Syncing,
    /// Normal operation.
    Live,
    /// Authority lost, read-only mode.
    Ghost,
}
