//! IRC connector for the Interconnect protocol.
//!
//! Presents an IRC channel as an Interconnect room. Clients receive channel
//! messages as snapshots and send intents that become IRC PRIVMSGs.
//!
//! Uses plain TCP (RFC 1459). No TLS — connect to port 6667.
//!
//! # Usage
//!
//! ```ignore
//! use interconnect_connector_irc as irc;
//!
//! let (mut conn, snapshot) = irc::connect("irc.libera.chat", 6667, "mybot", "#rust").await?;
//!
//! println!("Connected to {}", conn.manifest().name);
//! for msg in &snapshot.messages {
//!     println!("{}: {}", msg.nick, msg.text);
//! }
//!
//! // Send a message to the channel:
//! conn.send_intent(irc::IrcIntent::SendMessage {
//!     text: "hello from another room".to_string(),
//! }).await?;
//! ```

mod connector;
mod transport;
mod types;

pub use connector::{IrcConnection, connect};
pub use types::{IrcError, IrcIntent, IrcMessage, IrcSnapshot};
