//! Client-side implementation for the Interconnect protocol.
//!
//! # Quick Start
//!
//! ```ignore
//! use interconnect_client::{WsTransport, Connection};
//! use interconnect_core::Identity;
//!
//! let transport = WsTransport::connect("ws://localhost:8080").await?;
//! let (mut conn, snapshot) = Connection::<_, MyIntent, MySnapshot>::connect(
//!     transport,
//!     Identity::local("alice"),
//!     Some("Alice".to_string()),
//!     None,
//! ).await?;
//!
//! conn.send_intent(MyIntent::Hello).await?;
//!
//! while let Some(msg) = conn.recv().await? {
//!     // handle ServerWire::Snapshot, Transfer, Error, etc.
//! }
//! ```
//!
//! # Multiple Authorities
//!
//! A client can be connected to multiple authorities simultaneously.
//! Hold two `Connection` instances and use `tokio::select!`:
//!
//! ```ignore
//! loop {
//!     tokio::select! {
//!         msg = conn_a.recv() => { /* handle room A, maybe send intent to conn_b */ }
//!         msg = conn_b.recv() => { /* handle room B, maybe send intent to conn_a */ }
//!     }
//! }
//! ```

mod connection;
mod error;
mod transport;

pub use connection::Connection;
pub use error::ClientError;
pub use transport::WsTransport;

/// Convenience type alias for a WebSocket-backed connection.
pub type WsConnection<I, S> = Connection<WsTransport, I, S>;
