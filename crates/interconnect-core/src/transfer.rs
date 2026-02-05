//! Transfer types for server-to-server handoff.

use crate::Identity;
use serde::{Deserialize, Serialize};

/// A transfer directive, telling the client to connect to another server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transfer {
    /// Where to connect (app-defined format, typically a URL).
    pub destination: String,
    /// The passport to present to the destination.
    pub passport: Passport,
}

/// A passport carried during transfer.
///
/// Contains the user's identity and app-defined data that travels with them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Passport {
    /// The user's identity.
    pub identity: Identity,
    /// App-defined payload (inventory, stats, etc.).
    pub data: Vec<u8>,
    /// Optional signature (scheme-dependent).
    pub signature: Option<Vec<u8>>,
}

impl Passport {
    /// Create a new unsigned passport.
    pub fn new(identity: Identity, data: Vec<u8>) -> Self {
        Self {
            identity,
            data,
            signature: None,
        }
    }

    /// Create a passport with a signature.
    pub fn signed(identity: Identity, data: Vec<u8>, signature: Vec<u8>) -> Self {
        Self {
            identity,
            data,
            signature: Some(signature),
        }
    }
}
