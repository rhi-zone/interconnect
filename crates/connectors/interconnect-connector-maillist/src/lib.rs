//! Mailing list connector for the Interconnect protocol (Listmonk).
//!
//! Presents a Listmonk mailing list as an Interconnect room. Clients receive
//! campaign archives as snapshots and send intents that become Listmonk API
//! calls. New campaigns are discovered via polling every 30 seconds.

mod connector;
mod transport;
mod types;

pub use connector::{MailConnection, connect};
pub use types::{MailError, MailIntent, MailMessage, MailSnapshot};
