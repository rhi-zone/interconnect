//! WhatsApp Business Cloud API connector for the Interconnect protocol.
//!
//! Presents a WhatsApp conversation as an Interconnect room. Clients send
//! intents that become WhatsApp Cloud API calls; the authority broadcasts
//! conversation state as snapshots.
//!
//! # API used
//!
//! This crate uses the **official WhatsApp Business Cloud API** (Meta Graph API
//! v18.0). It requires:
//! - A Meta Business account with a verified WhatsApp Business App
//! - A phone number registered in the Meta Developer Console
//! - A `whatsapp_business_messaging`-scoped Graph API access token
//!
//! # Receiving messages (webhook limitation)
//!
//! The Cloud API delivers **inbound messages via webhooks**, not polling.
//! `recv()` currently returns `Ok(None)` — making this a send-only connector
//! until webhook support is wired up. See the TODO in `transport.rs` for the
//! integration path.
//!
//! # Personal WhatsApp
//!
//! For personal (non-Business) WhatsApp accounts, there is no official API.
//! Unofficial approaches such as [whatsmeow] (Go) or [Baileys] (Node.js)
//! reverse-engineer the WhatsApp Web protocol. These work but carry a risk of
//! account suspension; they are not used here.
//!
//! [whatsmeow]: https://github.com/tulir/whatsmeow
//! [Baileys]: https://github.com/WhiskeySockets/Baileys
//!
//! # Usage
//!
//! ```ignore
//! use interconnect_connector_whatsapp as whatsapp;
//!
//! let (mut conn, snapshot) = whatsapp::connect(
//!     "123456789012345",   // phone_number_id from Meta Developer Console
//!     "EAAxxxxxxx...",     // Graph API access token
//!     "15551234567",       // recipient in E.164 format
//! ).await?;
//!
//! println!("Connected via {}", conn.manifest().name);
//!
//! conn.send_intent(whatsapp::WhatsAppIntent::SendMessage {
//!     text: "hello from another room".to_string(),
//! }).await?;
//! ```

mod connector;
mod transport;
mod types;

pub use connector::{WhatsAppConnection, connect};
pub use types::{WhatsAppError, WhatsAppIntent, WhatsAppMessage, WhatsAppSnapshot};
