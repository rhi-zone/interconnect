//! SQLite-specific protocol types.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Column metadata from `PRAGMA table_info`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub type_name: String,
}

/// Snapshot of a SQLite table room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteSnapshot {
    /// Path to the SQLite database file.
    pub path: String,
    /// Name of the watched table or view.
    pub table: String,
    /// Current rows, each row as a map of column name to JSON value.
    pub rows: Vec<HashMap<String, serde_json::Value>>,
    /// Column schema for the table.
    pub schema: Vec<ColumnInfo>,
}

/// Intents a client can send to a SQLite table room.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SqliteIntent {
    /// Run arbitrary parameterised SQL (INSERT/UPDATE/DELETE).
    Execute {
        sql: String,
        params: Vec<serde_json::Value>,
    },
    /// Insert a row into the table.
    Insert {
        values: HashMap<String, serde_json::Value>,
    },
    /// Delete rows matching a WHERE clause.
    Delete {
        where_sql: String,
        params: Vec<serde_json::Value>,
    },
}

/// Errors from SQLite connector operations.
#[derive(Debug, thiserror::Error)]
pub enum SqliteError {
    #[error("sqlite error: {0}")]
    Rusqlite(#[from] rusqlite::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("sqlite error: {0}")]
    Other(String),
}

impl From<SqliteError> for interconnect_client::ClientError {
    fn from(e: SqliteError) -> Self {
        interconnect_client::ClientError::Other(Box::new(e))
    }
}
