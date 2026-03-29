//! High-level connector entry point.

use std::path::Path;
use std::sync::{Arc, Mutex};

use interconnect_client::Connection;
use interconnect_core::{Identity, Manifest};

use crate::transport::SqliteTransport;
use crate::types::{ColumnInfo, SqliteError, SqliteIntent, SqliteSnapshot};

pub type SqliteConnection = Connection<SqliteTransport, SqliteIntent, SqliteSnapshot>;

/// Connect to a SQLite table as an Interconnect room.
///
/// Opens (or creates) the database at `path` and watches `table`. Returns a
/// live connection and the initial snapshot of all current rows.
///
/// # Example
///
/// ```ignore
/// use interconnect_connector_sqlite as sqlite;
///
/// let (mut conn, snapshot) = sqlite::connect("my.db", "events").await?;
///
/// println!("Connected to {}.{}", snapshot.path, snapshot.table);
/// println!("{} rows", snapshot.rows.len());
///
/// conn.send_intent(sqlite::SqliteIntent::Insert {
///     values: [("name".into(), serde_json::json!("hello"))].into(),
/// }).await?;
/// ```
pub async fn connect(
    path: impl AsRef<Path>,
    table: impl Into<String>,
) -> Result<(SqliteConnection, SqliteSnapshot), SqliteError> {
    let path = path.as_ref().to_path_buf();
    let table = table.into();

    let path_clone = path.clone();
    let table_clone = table.clone();

    let (conn_raw, schema, initial_snapshot, sig) =
        tokio::task::spawn_blocking(move || -> Result<_, SqliteError> {
            let conn = rusqlite::Connection::open(&path_clone)?;

            // Introspect schema via PRAGMA table_info.
            let schema = {
                let mut stmt =
                    conn.prepare(&format!("PRAGMA table_info(\"{}\")", table_clone))?;
                let cols: Result<Vec<ColumnInfo>, _> = stmt
                    .query_map([], |row| {
                        Ok(ColumnInfo {
                            name: row.get::<_, String>(1)?,
                            type_name: row.get::<_, String>(2)?,
                        })
                    })?
                    .collect();
                cols?
            };

            let sig = SqliteTransport::read_signature(&conn, &table_clone)?;
            let snapshot =
                SqliteTransport::current_snapshot(&conn, &path_clone, &table_clone, &schema)?;

            Ok((conn, schema, snapshot, sig))
        })
        .await
        .map_err(|e| SqliteError::Other(e.to_string()))??;

    let db = Arc::new(Mutex::new(conn_raw));

    let transport = SqliteTransport {
        db,
        path: path.clone(),
        table: table.clone(),
        schema,
        seq: 0,
        last_signature: sig,
    };

    let manifest = Manifest {
        identity: Identity::local(format!("sqlite:{}:{}", path.display(), table)),
        name: format!("{table} ({})", path.display()),
        substrate: None,
        metadata: serde_json::json!({
            "type": "sqlite",
            "path": path.display().to_string(),
            "table": table,
        }),
    };

    let connection = SqliteConnection::established(transport, manifest);
    Ok((connection, initial_snapshot))
}
