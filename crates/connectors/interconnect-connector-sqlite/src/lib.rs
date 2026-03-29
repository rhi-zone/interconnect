//! SQLite connector for the Interconnect protocol.
//!
//! Presents a SQLite table or view as an Interconnect room. Clients receive
//! the current rows as snapshots and send intents that become SQL mutations.
//!
//! Uses [`rusqlite`] with the bundled feature (no system SQLite required).
//! Change detection polls every 2 seconds using `COUNT(*) / MAX(rowid)`.
//!
//! # Usage
//!
//! ```ignore
//! use interconnect_connector_sqlite as sqlite;
//!
//! let (mut conn, snapshot) = sqlite::connect("my.db", "events").await?;
//!
//! println!("Connected to {}.{}", snapshot.path, snapshot.table);
//! for row in &snapshot.rows {
//!     println!("{row:?}");
//! }
//!
//! // Insert a row:
//! conn.send_intent(sqlite::SqliteIntent::Insert {
//!     values: [
//!         ("name".into(), serde_json::json!("hello")),
//!         ("value".into(), serde_json::json!(42)),
//!     ].into(),
//! }).await?;
//! ```

mod chat;
mod connector;
mod transport;
mod types;

pub use chat::{
    ChatIntent, ChatLogConfig, ColType, ColumnMapping, SqliteChatConnection, SqliteChatSnapshot,
    connect_chat, extract,
};
pub use connector::{SqliteConnection, connect};
pub use types::{ColumnInfo, SqliteError, SqliteIntent, SqliteSnapshot};
