//! Filesystem connector for the Interconnect protocol.
//!
//! Presents a local directory as an Interconnect room. File changes become
//! snapshots; write/delete intents modify the filesystem.
//!
//! Text files only (UTF-8). Binary files, hidden files, and files over 1 MiB
//! are excluded from snapshots. Directory depth is limited to 5 levels.
//!
//! # Usage
//!
//! ```ignore
//! use interconnect_connector_fs as fs;
//!
//! let (mut conn, snapshot) = fs::connect("/home/alice/notes").await?;
//!
//! println!("Room: {} ({} files)", conn.manifest().name, snapshot.files.len());
//!
//! // Files arrive as snapshots when they change:
//! while let Some(msg) = conn.recv().await? {
//!     // ServerWire::Snapshot { data: FsSnapshot, .. }
//! }
//! ```

mod connector;
mod transport;
mod types;

pub use connector::{FsConnection, connect};
pub use types::{FsError, FsFile, FsIntent, FsSnapshot};
