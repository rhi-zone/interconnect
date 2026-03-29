//! High-level entry point for the filesystem connector.

use crate::transport::FsTransport;
use crate::types::{FsError, FsIntent, FsSnapshot};
use interconnect_client::Connection;
use interconnect_core::{Identity, Manifest};
use std::path::Path;

pub type FsConnection = Connection<FsTransport, FsIntent, FsSnapshot>;

/// Watch a directory as an Interconnect room.
///
/// Returns a live connection and the initial snapshot of the directory contents.
///
/// # Example
///
/// ```ignore
/// let (mut conn, snapshot) = fs::connect("/home/alice/notes").await?;
///
/// println!("{} files in room", snapshot.files.len());
///
/// // Write a file:
/// conn.send_intent(fs::FsIntent::WriteFile {
///     path: "hello.md".to_string(),
///     content: "# Hello\n".to_string(),
/// }).await?;
///
/// // Changes arrive as snapshots:
/// while let Some(msg) = conn.recv().await? {
///     // handle ServerWire::Snapshot { data: FsSnapshot, .. }
/// }
/// ```
pub async fn connect(root: impl AsRef<Path>) -> Result<(FsConnection, FsSnapshot), FsError> {
    let root = root.as_ref().canonicalize()?;

    let transport = FsTransport::new(root.clone())?;
    let initial_snapshot = transport.read_snapshot().await?;

    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_string_lossy().into_owned());

    let manifest = Manifest {
        identity: Identity::local(format!("fs:{}", root.to_string_lossy())),
        name,
        substrate: None,
        metadata: serde_json::json!({ "type": "fs", "root": root }),
    };

    let conn = FsConnection::established(transport, manifest);
    Ok((conn, initial_snapshot))
}
