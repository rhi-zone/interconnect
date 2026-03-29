//! Chat log mode for the SQLite connector.
//!
//! `connect_chat` opens (or creates) a SQLite database and manages a `messages`
//! table whose schema is defined by a [`ChatLogConfig`]. The table is created
//! automatically if absent.
//!
//! Callers receive a [`SqliteChatConnection`] and an initial [`SqliteChatSnapshot`].
//! The connection polls for new rows every 2 seconds and accepts [`ChatIntent`]
//! for inserts or raw SQL execution.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use interconnect_client::Connection;
use interconnect_core::{ClientWire, Identity, Manifest, ServerWire, Transport};
use serde::{Deserialize, Serialize};

use crate::types::SqliteError;

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/// SQLite type affinity for a column.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColType {
    Text,
    Integer,
    Real,
    Boolean,
    Json,
    Blob,
}

impl ColType {
    /// Return the SQLite type name used in DDL.
    fn as_sql_type(&self) -> &'static str {
        match self {
            ColType::Text => "TEXT",
            ColType::Integer => "INTEGER",
            ColType::Real => "REAL",
            ColType::Boolean => "INTEGER",
            ColType::Json => "TEXT",
            ColType::Blob => "BLOB",
        }
    }
}

/// Mapping from a snapshot JSON path to a table column.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMapping {
    /// Dot-path into the snapshot JSON. `"*"` serializes the entire snapshot.
    pub path: String,
    /// SQLite type affinity for this column.
    pub col_type: ColType,
    /// Whether this column is part of the PRIMARY KEY.
    #[serde(default)]
    pub primary_key: bool,
    /// Whether this column may be NULL.
    #[serde(default)]
    pub nullable: bool,
}

/// User-defined column configuration for a chat log table.
///
/// Can be constructed programmatically or deserialized from TOML:
///
/// ```toml
/// [chat_log.columns]
/// id     = { path = "id",     type = "text", primary_key = true }
/// author = { path = "author", type = "text" }
/// raw    = { path = "*",      type = "json" }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatLogConfig {
    /// Column definitions. Key = column name, value = mapping spec.
    pub columns: HashMap<String, ColumnMapping>,
}

impl ChatLogConfig {
    /// Default chat log configuration matching the standard `messages` schema:
    ///
    /// ```sql
    /// CREATE TABLE IF NOT EXISTS messages (
    ///     id        TEXT,
    ///     author    TEXT,
    ///     channel   TEXT,
    ///     server    TEXT,
    ///     text      TEXT,
    ///     timestamp INTEGER,
    ///     platform  TEXT,
    ///     raw       TEXT,
    ///     PRIMARY KEY (platform, id)
    /// )
    /// ```
    pub fn chat_default() -> Self {
        let mut columns = HashMap::new();

        columns.insert(
            "id".into(),
            ColumnMapping {
                path: "id".into(),
                col_type: ColType::Text,
                primary_key: true,
                nullable: false,
            },
        );
        columns.insert(
            "author".into(),
            ColumnMapping {
                path: "author".into(),
                col_type: ColType::Text,
                primary_key: false,
                nullable: false,
            },
        );
        columns.insert(
            "channel".into(),
            ColumnMapping {
                path: "channel".into(),
                col_type: ColType::Text,
                primary_key: false,
                nullable: true,
            },
        );
        columns.insert(
            "server".into(),
            ColumnMapping {
                path: "server".into(),
                col_type: ColType::Text,
                primary_key: false,
                nullable: true,
            },
        );
        columns.insert(
            "text".into(),
            ColumnMapping {
                path: "text".into(),
                col_type: ColType::Text,
                primary_key: false,
                nullable: false,
            },
        );
        columns.insert(
            "timestamp".into(),
            ColumnMapping {
                path: "timestamp".into(),
                col_type: ColType::Integer,
                primary_key: false,
                nullable: false,
            },
        );
        columns.insert(
            "platform".into(),
            ColumnMapping {
                path: "platform".into(),
                col_type: ColType::Text,
                primary_key: true,
                nullable: false,
            },
        );
        columns.insert(
            "raw".into(),
            ColumnMapping {
                path: "*".into(),
                col_type: ColType::Json,
                primary_key: false,
                nullable: true,
            },
        );

        ChatLogConfig { columns }
    }

    /// Build the `CREATE TABLE IF NOT EXISTS messages (...)` DDL from this config.
    pub(crate) fn build_ddl(&self) -> String {
        // Collect columns in a deterministic order: primary-key columns first,
        // then the rest, alphabetically within each group.
        let mut pk_cols: Vec<&str> = self
            .columns
            .iter()
            .filter(|(_, m)| m.primary_key)
            .map(|(name, _)| name.as_str())
            .collect();
        pk_cols.sort_unstable();

        let mut non_pk_cols: Vec<&str> = self
            .columns
            .iter()
            .filter(|(_, m)| !m.primary_key)
            .map(|(name, _)| name.as_str())
            .collect();
        non_pk_cols.sort_unstable();

        let all_cols: Vec<&str> = pk_cols.iter().chain(non_pk_cols.iter()).copied().collect();

        let mut col_defs: Vec<String> = all_cols
            .iter()
            .map(|name| {
                let m = &self.columns[*name];
                let not_null = if !m.nullable && !m.primary_key {
                    " NOT NULL"
                } else {
                    ""
                };
                format!("    {} {}{}", name, m.col_type.as_sql_type(), not_null)
            })
            .collect();

        // Composite PRIMARY KEY clause if more than one PK column, or single PK.
        if !pk_cols.is_empty() {
            let pk_list = pk_cols
                .iter()
                .map(|c| format!("\"{}\"", c))
                .collect::<Vec<_>>()
                .join(", ");
            col_defs.push(format!("    PRIMARY KEY ({})", pk_list));
        }

        format!(
            "CREATE TABLE IF NOT EXISTS messages (\n{}\n)",
            col_defs.join(",\n")
        )
    }
}

// ---------------------------------------------------------------------------
// JSON path extraction
// ---------------------------------------------------------------------------

/// Extract a value from a JSON snapshot by dot-path.
///
/// - `"*"` → returns the whole snapshot serialized as a JSON string.
/// - `"foo.bar.baz"` → walks object keys; returns `Null` if any step is missing.
pub fn extract(snapshot: &serde_json::Value, path: &str) -> serde_json::Value {
    if path == "*" {
        return serde_json::Value::String(snapshot.to_string());
    }

    let mut current = snapshot;
    for key in path.split('.') {
        match current.get(key) {
            Some(v) => current = v,
            None => return serde_json::Value::Null,
        }
    }
    current.clone()
}

// ---------------------------------------------------------------------------
// Snapshot and intent types
// ---------------------------------------------------------------------------

/// Snapshot of a chat log room: all rows from the `messages` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteChatSnapshot {
    /// Path to the SQLite database file.
    pub path: String,
    /// All rows, each as a map of column name → JSON value.
    pub rows: Vec<HashMap<String, serde_json::Value>>,
}

/// Intents a client can send to a chat log room.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatIntent {
    /// Insert a message. Values are keyed by column name and will be mapped
    /// through the config's column mappings before insertion.
    Insert {
        values: HashMap<String, serde_json::Value>,
    },
    /// Execute raw parameterised SQL (INSERT / UPDATE / DELETE).
    Execute {
        sql: String,
        params: Vec<serde_json::Value>,
    },
}

// ---------------------------------------------------------------------------
// Transport
// ---------------------------------------------------------------------------

const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);

pub struct SqliteChatTransport {
    db: Arc<Mutex<rusqlite::Connection>>,
    path: std::path::PathBuf,
    config: ChatLogConfig,
    seq: u64,
    /// Last observed (COUNT(*), MAX(rowid)) for cheap change detection.
    last_signature: (i64, i64),
}

impl SqliteChatTransport {
    fn read_signature(conn: &rusqlite::Connection) -> Result<(i64, i64), SqliteError> {
        let sig = conn
            .query_row(
                "SELECT COUNT(*), COALESCE(MAX(rowid), 0) FROM \"messages\"",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .unwrap_or((0, 0));
        Ok(sig)
    }

    fn read_all_rows(
        conn: &rusqlite::Connection,
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, SqliteError> {
        let mut stmt = conn.prepare("SELECT * FROM \"messages\"")?;
        let col_names: Vec<String> = stmt
            .column_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect();

        let rows: Result<Vec<_>, rusqlite::Error> = stmt
            .query_map([], |row| {
                let mut map = HashMap::new();
                for (i, name) in col_names.iter().enumerate() {
                    let val = value_ref_to_json(row.get_ref(i)?);
                    map.insert(name.clone(), val);
                }
                Ok(map)
            })?
            .collect();

        Ok(rows?)
    }
}

impl Transport for SqliteChatTransport {
    type Error = SqliteError;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let wire: ClientWire<ChatIntent> = serde_json::from_slice(data)?;

        let intent = match wire {
            ClientWire::Intent(i) => i,
            _ => return Ok(()),
        };

        let db = Arc::clone(&self.db);
        let config = self.config.clone();

        tokio::task::spawn_blocking(move || {
            let conn = db.lock().expect("sqlite mutex poisoned");
            match intent {
                ChatIntent::Insert { values } => {
                    // Build column/value lists. For each configured column,
                    // attempt to find the value in the provided map by column
                    // name directly (callers supply column names, not JSON paths).
                    if values.is_empty() {
                        return Ok(());
                    }
                    let cols: Vec<String> = values.keys().cloned().collect();
                    let placeholders: Vec<String> =
                        (1..=cols.len()).map(|i| format!("?{i}")).collect();
                    let sql = format!(
                        "INSERT OR REPLACE INTO \"messages\" ({cols}) VALUES ({placeholders})",
                        cols = cols
                            .iter()
                            .map(|c| format!("\"{c}\""))
                            .collect::<Vec<_>>()
                            .join(", "),
                        placeholders = placeholders.join(", "),
                    );
                    let owned: Vec<rusqlite::types::Value> = cols
                        .iter()
                        .map(|c| json_to_sql(values.get(c).unwrap_or(&serde_json::Value::Null)))
                        .collect();
                    let refs: Vec<&dyn rusqlite::types::ToSql> =
                        owned.iter().map(|v| v as &dyn rusqlite::types::ToSql).collect();
                    conn.execute(&sql, refs.as_slice())?;

                    let _ = config; // config available for future validation
                    Ok(())
                }
                ChatIntent::Execute { sql, params } => {
                    let owned: Vec<rusqlite::types::Value> =
                        params.iter().map(json_to_sql).collect();
                    let refs: Vec<&dyn rusqlite::types::ToSql> =
                        owned.iter().map(|v| v as &dyn rusqlite::types::ToSql).collect();
                    conn.execute(&sql, refs.as_slice())?;
                    Ok(())
                }
            }
        })
        .await
        .map_err(|e| SqliteError::Other(e.to_string()))?
        .map_err(SqliteError::Rusqlite)?;

        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<Vec<u8>>, Self::Error> {
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;

            let db = Arc::clone(&self.db);
            let path = self.path.clone();
            let last_sig = self.last_signature;

            let result = tokio::task::spawn_blocking(move || {
                let conn = db.lock().expect("sqlite mutex poisoned");
                let sig = SqliteChatTransport::read_signature(&conn)?;
                if sig == last_sig {
                    return Ok::<_, SqliteError>(None);
                }
                let rows = SqliteChatTransport::read_all_rows(&conn)?;
                let snapshot = SqliteChatSnapshot {
                    path: path.to_string_lossy().into_owned(),
                    rows,
                };
                Ok(Some((sig, snapshot)))
            })
            .await
            .map_err(|e| SqliteError::Other(e.to_string()))??;

            if let Some((new_sig, snapshot)) = result {
                self.last_signature = new_sig;
                let wire = ServerWire::<SqliteChatSnapshot>::Snapshot {
                    seq: self.seq,
                    data: snapshot,
                };
                self.seq += 1;
                return Ok(Some(serde_json::to_vec(&wire)?));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Public connection type and entry point
// ---------------------------------------------------------------------------

/// A live connection to a SQLite chat log room.
pub type SqliteChatConnection = Connection<SqliteChatTransport, ChatIntent, SqliteChatSnapshot>;

/// Open (or create) a SQLite chat log database.
///
/// Creates the `messages` table according to `config` if it does not already
/// exist, then returns a live connection and the initial snapshot of all
/// current rows.
///
/// # Example
///
/// ```ignore
/// use interconnect_connector_sqlite::{ChatLogConfig, connect_chat};
///
/// let (mut conn, snapshot) = connect_chat("chat.db", ChatLogConfig::chat_default()).await?;
///
/// println!("{} messages", snapshot.rows.len());
/// ```
pub async fn connect_chat(
    path: impl AsRef<Path>,
    config: ChatLogConfig,
) -> Result<(SqliteChatConnection, SqliteChatSnapshot), SqliteError> {
    let path = path.as_ref().to_path_buf();
    let ddl = config.build_ddl();
    let path_clone = path.clone();

    let (conn_raw, initial_snapshot, sig) =
        tokio::task::spawn_blocking(move || -> Result<_, SqliteError> {
            let conn = rusqlite::Connection::open(&path_clone)?;
            conn.execute_batch(&ddl)?;

            let sig = SqliteChatTransport::read_signature(&conn)?;
            let rows = SqliteChatTransport::read_all_rows(&conn)?;
            let snapshot = SqliteChatSnapshot {
                path: path_clone.to_string_lossy().into_owned(),
                rows,
            };

            Ok((conn, snapshot, sig))
        })
        .await
        .map_err(|e| SqliteError::Other(e.to_string()))??;

    let db = Arc::new(Mutex::new(conn_raw));

    let transport = SqliteChatTransport {
        db,
        path: path.clone(),
        config,
        seq: 0,
        last_signature: sig,
    };

    let manifest = Manifest {
        identity: Identity::local(format!("sqlite:{}:messages", path.display())),
        name: format!("messages ({})", path.display()),
        substrate: None,
        metadata: serde_json::json!({
            "type": "sqlite_chat",
            "path": path.display().to_string(),
            "table": "messages",
        }),
    };

    let connection = SqliteChatConnection::established(transport, manifest);
    Ok((connection, initial_snapshot))
}

// ---------------------------------------------------------------------------
// Helpers (duplicated from transport.rs to keep this module self-contained)
// ---------------------------------------------------------------------------

fn value_ref_to_json(v: rusqlite::types::ValueRef<'_>) -> serde_json::Value {
    use rusqlite::types::ValueRef;
    match v {
        ValueRef::Null => serde_json::Value::Null,
        ValueRef::Integer(i) => serde_json::Value::Number(i.into()),
        ValueRef::Real(f) => serde_json::json!(f),
        ValueRef::Text(t) => serde_json::Value::String(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => {
            let hex: String = b.iter().map(|byte| format!("{byte:02x}")).collect();
            serde_json::Value::String(format!("0x{hex}"))
        }
    }
}

fn json_to_sql(v: &serde_json::Value) -> rusqlite::types::Value {
    match v {
        serde_json::Value::Null => rusqlite::types::Value::Null,
        serde_json::Value::Bool(b) => rusqlite::types::Value::Integer(*b as i64),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                rusqlite::types::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                rusqlite::types::Value::Real(f)
            } else {
                rusqlite::types::Value::Null
            }
        }
        serde_json::Value::String(s) => rusqlite::types::Value::Text(s.clone()),
        other => rusqlite::types::Value::Text(other.to_string()),
    }
}
