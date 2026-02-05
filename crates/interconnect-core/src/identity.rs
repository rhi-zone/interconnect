//! Algorithm-agnostic identity.
//!
//! Identity format: `scheme:payload`
//!
//! Supported schemes:
//! - `local:name` - Trust the connection (dev/LAN)
//! - `url:user@server` - Server vouches for user
//! - `ed25519:fingerprint` - Cryptographic (user holds key)

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// An identity in the form `scheme:payload`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Identity {
    scheme: String,
    payload: String,
}

impl Identity {
    /// Create a new identity.
    pub fn new(scheme: impl Into<String>, payload: impl Into<String>) -> Self {
        Self {
            scheme: scheme.into(),
            payload: payload.into(),
        }
    }

    /// Create a local (trust-the-connection) identity.
    pub fn local(name: impl Into<String>) -> Self {
        Self::new("local", name)
    }

    /// Create a URL-based (server-vouched) identity.
    pub fn url(user_at_server: impl Into<String>) -> Self {
        Self::new("url", user_at_server)
    }

    /// The scheme (e.g., "local", "url", "ed25519").
    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    /// The payload (interpretation depends on scheme).
    pub fn payload(&self) -> &str {
        &self.payload
    }

    /// Check if this is a local (unverified) identity.
    pub fn is_local(&self) -> bool {
        self.scheme == "local"
    }
}

impl fmt::Display for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.scheme, self.payload)
    }
}

impl FromStr for Identity {
    type Err = IdentityParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (scheme, payload) = s
            .split_once(':')
            .ok_or_else(|| IdentityParseError::MissingColon(s.to_string()))?;

        if scheme.is_empty() {
            return Err(IdentityParseError::EmptyScheme);
        }

        Ok(Self {
            scheme: scheme.to_string(),
            payload: payload.to_string(),
        })
    }
}

impl TryFrom<String> for Identity {
    type Error = IdentityParseError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<Identity> for String {
    fn from(id: Identity) -> Self {
        id.to_string()
    }
}

/// Error parsing an identity string.
#[derive(Debug, Clone, thiserror::Error)]
pub enum IdentityParseError {
    #[error("identity must contain ':' separator, got: {0}")]
    MissingColon(String),
    #[error("identity scheme cannot be empty")]
    EmptyScheme,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_local() {
        let id: Identity = "local:alice".parse().unwrap();
        assert_eq!(id.scheme(), "local");
        assert_eq!(id.payload(), "alice");
        assert!(id.is_local());
    }

    #[test]
    fn parse_url() {
        let id: Identity = "url:alice@example.com".parse().unwrap();
        assert_eq!(id.scheme(), "url");
        assert_eq!(id.payload(), "alice@example.com");
        assert!(!id.is_local());
    }

    #[test]
    fn roundtrip() {
        let id = Identity::local("bob");
        let s = id.to_string();
        let id2: Identity = s.parse().unwrap();
        assert_eq!(id, id2);
    }
}
