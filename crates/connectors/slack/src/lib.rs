//! Slack connector for the Interconnect protocol.
//!
//! Presents a Slack channel as an Interconnect room. Clients receive channel
//! messages as snapshots and send intents that become Slack API calls.
//!
//! Uses Slack Socket Mode for receiving events (no public endpoint required)
//! and the Web API for sending messages.
//!
//! # Usage
//!
//! ```ignore
//! use interconnect_connector_slack as slack;
//!
//! let (mut conn, snapshot) = slack::connect(bot_token, app_token, "C1234567890").await?;
//!
//! println!("Connected to #{}", conn.manifest().name);
//! for msg in &snapshot.messages {
//!     println!("{}: {}", msg.user_name, msg.text);
//! }
//!
//! // Relay from another room into Slack:
//! conn.send_intent(slack::SlackIntent::SendMessage {
//!     text: "hello from another room".to_string(),
//! }).await?;
//! ```

mod connector;
mod transport;
mod types;

pub use connector::{SlackConnection, connect};
pub use types::{SlackError, SlackIntent, SlackMessage, SlackSnapshot};
