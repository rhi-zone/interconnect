//! Core types and traits for Interconnect.
//!
//! This crate provides the protocol primitives. Applications define their own
//! Intent, Snapshot, and Passport types; this crate provides the framing.
//!
//! # Quick Start
//!
//! 1. Define your types (Intent, Snapshot, Passport)
//! 2. Implement [`SimpleAuthority`] or [`Authority`]
//! 3. Use a transport crate to run your server
//!
//! # Example
//!
//! ```ignore
//! use interconnect_core::{SimpleAuthority, Session, ImportResult};
//!
//! struct MyServer { /* ... */ }
//!
//! impl SimpleAuthority for MyServer {
//!     type Intent = MyIntent;
//!     type Snapshot = MySnapshot;
//!     type Passport = MyPassport;
//!     type Error = MyError;
//!
//!     fn on_connect(&mut self, session: &Session) -> Result<(), Self::Error> { /* ... */ }
//!     fn on_transfer_in(&mut self, session: &Session, passport: MyPassport)
//!         -> Result<ImportResult<MyPassport>, Self::Error> { /* ... */ }
//!     fn on_disconnect(&mut self, session: &Session) { /* ... */ }
//!     fn handle_intent(&mut self, session: &Session, intent: MyIntent)
//!         -> Result<(), Self::Error> { /* ... */ }
//!     fn snapshot(&self) -> MySnapshot { /* ... */ }
//!     fn emit_passport(&self, session: &Session) -> MyPassport { /* ... */ }
//!     fn validate_destination(&self, destination: &str) -> bool { /* ... */ }
//! }
//! ```

mod authority;
mod identity;
mod message;
mod transfer;
mod transport;
mod wire;

pub use authority::{Authority, ImportResult, Rejection, Session, SimpleAuthority};
pub use identity::Identity;
pub use message::{ClientMessage, ServerMessage};
pub use transfer::{Passport, Transfer};
pub use transport::Transport;
pub use wire::{from_json, from_json_str, to_json, to_json_string, ClientWire, ServerWire, Wire};

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
