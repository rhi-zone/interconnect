//! High-level connector entry point.

use crate::transport::MatrixTransport;
use crate::types::{MatrixError, MatrixIntent, MatrixMessage, MatrixSnapshot};
use interconnect_client::Connection;
use interconnect_core::{Identity, Manifest};
use reqwest::Client;
use std::collections::VecDeque;

pub type MatrixConnection = Connection<MatrixTransport, MatrixIntent, MatrixSnapshot>;

/// Connect to a Matrix room as an Interconnect room.
///
/// Authenticates with `access_token` (already obtained externally) and
/// performs an initial sync to populate the message history.
///
/// Returns a live connection and the initial snapshot.
///
/// # Example
///
/// ```ignore
/// let (mut conn, snapshot) = matrix::connect(
///     "https://matrix.example.org",
///     "syt_...",
///     "!abc123:example.org",
/// ).await?;
///
/// println!("Connected to {}", conn.manifest().name);
/// for msg in &snapshot.messages {
///     println!("{}: {}", msg.sender, msg.body);
/// }
///
/// conn.send_intent(matrix::MatrixIntent::SendMessage {
///     text: "hello from another room".to_string(),
/// }).await?;
/// ```
pub async fn connect(
    homeserver: impl Into<String>,
    access_token: impl Into<String>,
    room_id: impl Into<String>,
) -> Result<(MatrixConnection, MatrixSnapshot), MatrixError> {
    let homeserver = homeserver.into();
    let access_token = access_token.into();
    let room_id = room_id.into();

    let http = Client::new();

    // Fetch room name for the manifest.
    let room_name = fetch_room_name(&http, &homeserver, &access_token, &room_id).await?;

    // Perform an initial sync without a `since` token to get recent history.
    let (initial_messages, since) =
        fetch_initial_messages(&http, &homeserver, &access_token, &room_id).await?;

    let mut messages: VecDeque<MatrixMessage> = VecDeque::new();
    for msg in initial_messages {
        messages.push_back(msg);
    }

    let transport = MatrixTransport {
        http,
        homeserver: homeserver.clone(),
        access_token: access_token.clone(),
        room_id: room_id.clone(),
        room_name: room_name.clone(),
        since: Some(since),
        messages,
        seq: 0,
        txn_counter: 0,
    };

    let snapshot = transport.current_snapshot();

    let manifest = Manifest {
        identity: Identity::local(format!("matrix:{room_id}")),
        name: room_name,
        substrate: None,
        metadata: serde_json::json!({
            "type": "matrix",
            "room_id": room_id,
            "homeserver": homeserver,
        }),
    };

    let conn = MatrixConnection::established(transport, manifest);
    Ok((conn, snapshot))
}

/// Fetch the canonical alias or name for a Matrix room via `GET /rooms/{roomId}/state/m.room.name`.
async fn fetch_room_name(
    http: &Client,
    homeserver: &str,
    access_token: &str,
    room_id: &str,
) -> Result<String, MatrixError> {
    let encoded = urlencoding_encode(room_id);
    let url = format!("{homeserver}/_matrix/client/v3/rooms/{encoded}/state/m.room.name");

    let resp: serde_json::Value = http
        .get(&url)
        .bearer_auth(access_token)
        .send()
        .await?
        .json()
        .await?;

    // `resp["name"]` contains the room display name when present.
    let name = resp["name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or(room_id)
        .to_string();

    Ok(name)
}

/// Perform a single sync with no `since` token and a short timeout to obtain
/// recent timeline events for the target room.
///
/// Returns `(messages, next_batch)`.
async fn fetch_initial_messages(
    http: &Client,
    homeserver: &str,
    access_token: &str,
    room_id: &str,
) -> Result<(Vec<MatrixMessage>, String), MatrixError> {
    // Use a filter that asks for the last 50 messages from our room only.
    let filter = serde_json::json!({
        "room": {
            "rooms": [room_id],
            "timeline": {
                "types": ["m.room.message"],
                "limit": 50
            }
        },
        "presence": { "types": [] },
        "account_data": { "types": [] }
    });
    let filter_str = serde_json::to_string(&filter)?;

    let resp: serde_json::Value = http
        .get(format!("{homeserver}/_matrix/client/v3/sync"))
        .bearer_auth(access_token)
        .query(&[("filter", filter_str.as_str()), ("timeout", "0")])
        .send()
        .await?
        .json()
        .await?;

    let next_batch = resp["next_batch"]
        .as_str()
        .ok_or_else(|| MatrixError::Api("missing next_batch in initial sync".into()))?
        .to_string();

    let events = resp["rooms"]["join"][room_id]["timeline"]["events"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let mut messages = Vec::new();
    for ev in &events {
        if ev["type"].as_str() != Some("m.room.message") {
            continue;
        }
        let content = &ev["content"];
        if content["msgtype"].as_str() != Some("m.text") {
            continue;
        }
        let event_id = ev["event_id"].as_str().unwrap_or("").to_string();
        let sender = ev["sender"].as_str().unwrap_or("").to_string();
        let body = content["body"].as_str().unwrap_or("").to_string();
        let timestamp = ev["origin_server_ts"].as_u64().unwrap_or(0);

        messages.push(MatrixMessage {
            event_id,
            sender,
            body,
            timestamp,
        });
    }

    Ok((messages, next_batch))
}

/// Percent-encode a string for use in a URL path segment (mirror of the one in
/// `transport.rs` — kept local to avoid exposing an internal helper).
fn urlencoding_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b => {
                out.push('%');
                out.push(char::from_digit((b >> 4) as u32, 16).unwrap());
                out.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
            }
        }
    }
    out
}
