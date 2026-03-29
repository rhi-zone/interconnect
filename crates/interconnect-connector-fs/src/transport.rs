//! Filesystem transport — watches a directory and presents it as a room.

use crate::types::{FsError, FsFile, FsIntent, FsSnapshot};
use interconnect_core::{ClientWire, ServerWire, Transport};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// Maximum file size to include in snapshots (1 MiB).
const MAX_FILE_SIZE: u64 = 1_048_576;
/// Maximum directory depth to walk.
const MAX_DEPTH: usize = 5;

pub struct FsTransport {
    root: PathBuf,
    event_rx: mpsc::UnboundedReceiver<notify::Event>,
    /// Kept alive to maintain the watch; dropped when the transport is dropped.
    _watcher: RecommendedWatcher,
    seq: u64,
}

impl FsTransport {
    pub fn new(root: PathBuf) -> Result<Self, FsError> {
        let (tx, event_rx) = mpsc::unbounded_channel();

        let mut watcher = notify::recommended_watcher(
            move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    let _ = tx.send(event);
                }
            },
        )?;

        watcher.watch(&root, RecursiveMode::Recursive)?;

        Ok(Self { root, event_rx, _watcher: watcher, seq: 0 })
    }

    /// Read the full directory snapshot (sync, runs in spawn_blocking).
    pub(crate) async fn read_snapshot(&self) -> Result<FsSnapshot, FsError> {
        let root = self.root.clone();
        let files = tokio::task::spawn_blocking(move || read_directory(&root)).await??;
        Ok(FsSnapshot { root: self.root.to_string_lossy().into_owned(), files })
    }
}

/// Synchronously walk the directory and collect readable text files.
fn read_directory(root: &Path) -> Result<Vec<FsFile>, FsError> {
    let mut files = Vec::new();

    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .max_depth(MAX_DEPTH)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();

        // Skip hidden files/directories (any component starting with '.').
        let rel = match path.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if rel.components().any(|c| {
            c.as_os_str().to_string_lossy().starts_with('.')
        }) {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let size = metadata.len();
        if size > MAX_FILE_SIZE {
            continue;
        }

        // Only include files that are valid UTF-8 text.
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        files.push(FsFile {
            path: rel.to_string_lossy().into_owned(),
            content,
            modified,
            size,
        });
    }

    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

impl Transport for FsTransport {
    type Error = FsError;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let wire: ClientWire<FsIntent> = serde_json::from_slice(data)?;
        match wire {
            ClientWire::Intent(FsIntent::WriteFile { path, content }) => {
                let full = self.root.join(&path);
                if let Some(parent) = full.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                tokio::fs::write(full, content).await?;
            }
            ClientWire::Intent(FsIntent::DeleteFile { path }) => {
                let full = self.root.join(&path);
                tokio::fs::remove_file(full).await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<Vec<u8>>, Self::Error> {
        loop {
            match self.event_rx.recv().await {
                None => return Ok(None),
                Some(event) => {
                    use notify::EventKind::*;
                    match event.kind {
                        Create(_) | Modify(_) | Remove(_) => {}
                        // Skip access, metadata-only, and other events.
                        _ => continue,
                    }

                    let snapshot = self.read_snapshot().await?;
                    let wire = ServerWire::<FsSnapshot>::Snapshot {
                        seq: self.seq,
                        data: snapshot,
                    };
                    self.seq += 1;
                    return Ok(Some(serde_json::to_vec(&wire)?));
                }
            }
        }
    }
}
