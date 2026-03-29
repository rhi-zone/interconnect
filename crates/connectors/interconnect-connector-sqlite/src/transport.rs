//! SQLite transport.
//!
//! Presents a SQLite table as an Interconnect `Transport`. Polling detects
//! row-count or rowid changes and emits `ServerWire<SqliteSnapshot>` bytes;
//! `ClientWire<SqliteIntent>` bytes become SQL mutations.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use interconnect_core::{ClientWire, ServerWire, Transport};
use rusqlite::types::ValueRef;

use crate::types::{ColumnInfo, SqliteError, SqliteIntent, SqliteSnapshot};

/// Poll interval for change detection.
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);

pub struct SqliteTransport {
    pub(crate) db: Arc<Mutex<rusqlite::Connection>>,
    pub(crate) path: PathBuf,
    pub(crate) table: String,
    pub(crate) schema: Vec<ColumnInfo>,
    pub(crate) seq: u64,
    /// Last observed (count, max_rowid) for cheap change detection.
    pub(crate) last_signature: (i64, i64),
}

impl SqliteTransport {
    /// Read all rows from the watched table.
    fn read_rows(
        conn: &rusqlite::Connection,
        table: &str,
        _schema: &[ColumnInfo],
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, SqliteError> {
        let sql = format!("SELECT * FROM \"{table}\"");
        let mut stmt = conn.prepare(&sql)?;
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

    /// Read the change-detection signature: (COUNT(*), MAX(rowid)).
    pub(crate) fn read_signature(
        conn: &rusqlite::Connection,
        table: &str,
    ) -> Result<(i64, i64), SqliteError> {
        // Views may not have rowid; fall back to 0 on error.
        let sig = conn
            .query_row(
                &format!("SELECT COUNT(*), COALESCE(MAX(rowid), 0) FROM \"{table}\""),
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .unwrap_or((0, 0));
        Ok(sig)
    }

    pub(crate) fn current_snapshot(
        conn: &rusqlite::Connection,
        path: &std::path::Path,
        table: &str,
        schema: &[ColumnInfo],
    ) -> Result<SqliteSnapshot, SqliteError> {
        let rows = Self::read_rows(conn, table, schema)?;
        Ok(SqliteSnapshot {
            path: path.to_string_lossy().into_owned(),
            table: table.to_string(),
            rows,
            schema: schema.to_vec(),
        })
    }
}

/// Convert a rusqlite `ValueRef` to a `serde_json::Value`.
fn value_ref_to_json(v: ValueRef<'_>) -> serde_json::Value {
    match v {
        ValueRef::Null => serde_json::Value::Null,
        ValueRef::Integer(i) => serde_json::Value::Number(i.into()),
        ValueRef::Real(f) => serde_json::json!(f),
        ValueRef::Text(t) => {
            serde_json::Value::String(String::from_utf8_lossy(t).into_owned())
        }
        ValueRef::Blob(b) => {
            // Encode blobs as a hex string prefixed with "0x".
            let hex: String = b.iter().map(|byte| format!("{byte:02x}")).collect();
            serde_json::Value::String(format!("0x{hex}"))
        }
    }
}

/// Convert a `serde_json::Value` to a rusqlite owned value for binding.
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

impl Transport for SqliteTransport {
    type Error = SqliteError;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let wire: ClientWire<SqliteIntent> = serde_json::from_slice(data)?;

        let intent = match wire {
            ClientWire::Intent(i) => i,
            // Auth, Ping, TransferRequest — not applicable for local connectors.
            _ => return Ok(()),
        };

        let db: Arc<Mutex<rusqlite::Connection>> = Arc::clone(&self.db);
        let table = self.table.clone();

        tokio::task::spawn_blocking(move || {
            let conn = db.lock().expect("sqlite mutex poisoned");
            match intent {
                SqliteIntent::Execute { sql, params } => {
                    let owned: Vec<rusqlite::types::Value> =
                        params.iter().map(json_to_sql).collect();
                    let refs: Vec<&dyn rusqlite::types::ToSql> =
                        owned.iter().map(|v| v as &dyn rusqlite::types::ToSql).collect();
                    conn.execute(&sql, refs.as_slice())?;
                }
                SqliteIntent::Insert { values } => {
                    if values.is_empty() {
                        return Ok(());
                    }
                    let cols: Vec<String> = values.keys().cloned().collect();
                    let placeholders: Vec<String> =
                        (1..=cols.len()).map(|i| format!("?{i}")).collect();
                    let sql = format!(
                        "INSERT INTO \"{table}\" ({cols}) VALUES ({placeholders})",
                        cols = cols
                            .iter()
                            .map(|c| format!("\"{c}\""))
                            .collect::<Vec<_>>()
                            .join(", "),
                        placeholders = placeholders.join(", "),
                    );
                    let owned: Vec<rusqlite::types::Value> =
                        cols.iter().map(|c| json_to_sql(&values[c])).collect();
                    let refs: Vec<&dyn rusqlite::types::ToSql> =
                        owned.iter().map(|v| v as &dyn rusqlite::types::ToSql).collect();
                    conn.execute(&sql, refs.as_slice())?;
                }
                SqliteIntent::Delete { where_sql, params } => {
                    let sql = format!("DELETE FROM \"{table}\" WHERE {where_sql}");
                    let owned: Vec<rusqlite::types::Value> =
                        params.iter().map(json_to_sql).collect();
                    let refs: Vec<&dyn rusqlite::types::ToSql> =
                        owned.iter().map(|v| v as &dyn rusqlite::types::ToSql).collect();
                    conn.execute(&sql, refs.as_slice())?;
                }
            }
            Ok::<(), rusqlite::Error>(())
        })
        .await
        .map_err(|e| SqliteError::Other(e.to_string()))?
        .map_err(SqliteError::Rusqlite)?;

        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<Vec<u8>>, Self::Error> {
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;

            let db: Arc<Mutex<rusqlite::Connection>> = Arc::clone(&self.db);
            let table = self.table.clone();
            let path = self.path.clone();
            let schema = self.schema.clone();
            let last_sig = self.last_signature;

            let result = tokio::task::spawn_blocking(move || {
                let conn = db.lock().expect("sqlite mutex poisoned");
                let sig = SqliteTransport::read_signature(&conn, &table)?;
                if sig == last_sig {
                    return Ok::<_, SqliteError>(None);
                }
                let snapshot =
                    SqliteTransport::current_snapshot(&conn, &path, &table, &schema)?;
                Ok(Some((sig, snapshot)))
            })
            .await
            .map_err(|e| SqliteError::Other(e.to_string()))??;

            if let Some((new_sig, snapshot)) = result {
                self.last_signature = new_sig;
                let wire = ServerWire::<SqliteSnapshot>::Snapshot {
                    seq: self.seq,
                    data: snapshot,
                };
                self.seq += 1;
                return Ok(Some(serde_json::to_vec(&wire)?));
            }
            // No change — loop and poll again.
        }
    }
}
