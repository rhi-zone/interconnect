//! Discord connector for the Interconnect protocol.
//!
//! Presents a Discord channel as an Interconnect room. Clients receive channel
//! messages as snapshots and send intents that become Discord API calls.
//!
//! # Usage
//!
//! ```ignore
//! use interconnect_connector_discord as discord;
//! use twilight_model::id::Id;
//!
//! let channel_id = Id::new(123456789);
//! let (mut conn, snapshot) = discord::connect(token, channel_id).await?;
//!
//! println!("Connected to #{}", conn.manifest().name);
//! for msg in &snapshot.messages {
//!     println!("{}: {}", msg.author_name, msg.content);
//! }
//!
//! // Relay from another room into Discord:
//! conn.send_intent(discord::DiscordIntent::SendMessage {
//!     content: "hello from another room".to_string(),
//! }).await?;
//! ```

mod connector;
mod transport;
mod types;

pub use connector::{DiscordConnection, connect};
pub use types::{DiscordError, DiscordIntent, DiscordMessage, DiscordSnapshot};

// Re-export the channel ID type so callers don't need to depend on twilight directly.
pub use twilight_model::id::{Id, marker::ChannelMarker};
