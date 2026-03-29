//! Zulip connector for the Interconnect protocol.
//!
//! Presents a Zulip stream/topic as an Interconnect room. Clients receive
//! messages as snapshots and send intents that become Zulip API calls.
//!
//! Zulip uses HTTP long polling for real-time events — the `recv()` call blocks
//! until a new message arrives in the configured stream/topic.
//!
//! # Usage
//!
//! ```ignore
//! use interconnect_connector_zulip as zulip;
//!
//! let (mut conn, snapshot) = zulip::connect(
//!     "https://chat.zulip.org",
//!     "bot@example.com",
//!     "api_key_here",
//!     "general",
//!     "announcements",
//! ).await?;
//!
//! println!("Connected to {}/{}", snapshot.stream, snapshot.topic);
//! for msg in &snapshot.messages {
//!     println!("{}: {}", msg.sender_name, msg.content);
//! }
//!
//! // Relay from another room into Zulip:
//! conn.send_intent(zulip::ZulipIntent::SendMessage {
//!     content: "hello from another room".to_string(),
//! }).await?;
//! ```

mod connector;
mod transport;
mod types;

pub use connector::{ZulipConnection, connect};
pub use types::{ZulipError, ZulipIntent, ZulipMessage, ZulipSnapshot};
