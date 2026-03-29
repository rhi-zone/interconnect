//! Matrix connector for the Interconnect protocol.
//!
//! Presents a Matrix room as an Interconnect room. Clients receive room
//! messages as snapshots and send intents that become Matrix API calls.
//!
//! Uses the Matrix Client-Server API directly via long-poll sync — no
//! heavyweight SDK required.
//!
//! # Usage
//!
//! ```ignore
//! use interconnect_connector_matrix as matrix;
//!
//! let (mut conn, snapshot) = matrix::connect(
//!     "https://matrix.example.org",
//!     "syt_access_token",
//!     "!roomid:example.org",
//! ).await?;
//!
//! println!("Connected to {}", conn.manifest().name);
//! for msg in &snapshot.messages {
//!     println!("{}: {}", msg.sender, msg.body);
//! }
//!
//! // Send a message into the Matrix room:
//! conn.send_intent(matrix::MatrixIntent::SendMessage {
//!     text: "hello from another room".to_string(),
//! }).await?;
//! ```

mod connector;
mod transport;
mod types;

pub use connector::{MatrixConnection, connect};
pub use types::{MatrixError, MatrixIntent, MatrixMessage, MatrixSnapshot};
