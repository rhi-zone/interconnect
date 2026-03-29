//! Filesystem connector types.

use serde::{Deserialize, Serialize};

/// A text file in the directory room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsFile {
    /// Relative path from the room root.
    pub path: String,
    /// File content (text files only; binary files are excluded).
    pub content: String,
    /// Last modified timestamp (Unix seconds).
    pub modified: u64,
    /// File size in bytes.
    pub size: u64,
}

/// Snapshot of a filesystem directory room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsSnapshot {
    /// Absolute path of the watched directory.
    pub root: String,
    /// All readable text files, sorted by path.
    pub files: Vec<FsFile>,
}

/// Intents a client can send to a filesystem room.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FsIntent {
    /// Write content to a file (creates or overwrites). Parent dirs created as needed.
    WriteFile { path: String, content: String },
    /// Delete a file.
    DeleteFile { path: String },
}

/// Errors from filesystem connector operations.
#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("watch error: {0}")]
    Watch(notify::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("task join error: {0}")]
    Join(#[from] tokio::task::JoinError),
}

impl From<notify::Error> for FsError {
    fn from(e: notify::Error) -> Self {
        FsError::Watch(e)
    }
}

impl From<FsError> for interconnect_client::ClientError {
    fn from(e: FsError) -> Self {
        interconnect_client::ClientError::Other(Box::new(e))
    }
}
