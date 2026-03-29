//! Telegram connector for the Interconnect protocol.
//!
//! Presents a Telegram chat as an Interconnect room. Clients receive chat
//! messages as snapshots and send intents that become Telegram Bot API calls.
//!
//! Uses the Telegram Bot API with long polling (`getUpdates`) — no webhook or
//! public endpoint required.
//!
//! # Usage
//!
//! ```ignore
//! use interconnect_connector_telegram as tg;
//!
//! let (mut conn, snapshot) = tg::connect("123456:ABC-DEF", -1001234567890_i64).await?;
//!
//! println!("Connected to {}", conn.manifest().name);
//! for msg in &snapshot.messages {
//!     println!("{}: {}", msg.from, msg.text);
//! }
//!
//! // Relay from another room into Telegram:
//! conn.send_intent(tg::TelegramIntent::SendMessage {
//!     text: "hello from another room".to_string(),
//! }).await?;
//! ```

mod connector;
mod transport;
mod types;

pub use connector::{TelegramConnection, connect};
pub use types::{TelegramError, TelegramIntent, TelegramMessage, TelegramSnapshot};
