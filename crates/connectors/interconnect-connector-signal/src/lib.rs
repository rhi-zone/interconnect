//! Signal connector for the Interconnect protocol.
//!
//! Presents a Signal conversation as an Interconnect room. Clients receive
//! messages as snapshots and send intents that become signal-cli JSON-RPC calls.
//!
//! # Requirements
//!
//! - Requires `signal-cli` installed and a registered Signal account. See
//!   <https://github.com/AsamK/signal-cli> for installation and registration.
//! - End-to-end encryption (E2EE) is handled transparently by signal-cli; this
//!   connector never handles encryption keys directly.
//! - Group chats are supported by passing the group ID (prefixed with `"group."`)
//!   as the `recipient` parameter to [`connect`].
//!
//! # Usage
//!
//! ```ignore
//! use interconnect_connector_signal as signal;
//!
//! let (mut conn, snapshot) = signal::connect(
//!     "/usr/bin/signal-cli",
//!     "+15550001234",   // your registered account
//!     "+15559876543",   // recipient phone number (or "group.<id>")
//! ).await?;
//!
//! for msg in &snapshot.messages {
//!     println!("{}: {}", msg.sender, msg.text);
//! }
//!
//! // Relay a message into Signal:
//! conn.send_intent(signal::SignalIntent::SendMessage {
//!     text: "hello from another room".to_string(),
//! }).await?;
//! ```

mod connector;
mod transport;
mod types;

pub use connector::{SignalConnection, connect};
pub use types::{SignalError, SignalIntent, SignalMessage, SignalSnapshot};
