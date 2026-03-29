//! High-level connector entry point.

use crate::transport::{ZulipTransport, register_event_queue};
use crate::types::{ZulipError, ZulipIntent, ZulipSnapshot};
use interconnect_client::Connection;
use interconnect_core::{Identity, Manifest};
use reqwest::Client;
use std::collections::VecDeque;

pub type ZulipConnection = Connection<ZulipTransport, ZulipIntent, ZulipSnapshot>;

/// Connect to a Zulip stream/topic as an Interconnect room.
///
/// Returns a live connection and the initial snapshot (recent message history).
///
/// # Example
///
/// ```ignore
/// let (mut conn, snapshot) = zulip::connect(realm, email, api_key, stream, topic).await?;
///
/// println!("Connected to {}/{}", snapshot.stream, snapshot.topic);
/// for msg in &snapshot.messages {
///     println!("{}: {}", msg.sender_name, msg.content);
/// }
///
/// // Relay from another room into Zulip:
/// conn.send_intent(zulip::ZulipIntent::SendMessage {
///     content: "hello from another room".to_string(),
/// }).await?;
/// ```
pub async fn connect(
    realm: impl Into<String>,
    email: impl Into<String>,
    api_key: impl Into<String>,
    stream: impl Into<String>,
    topic: impl Into<String>,
) -> Result<(ZulipConnection, ZulipSnapshot), ZulipError> {
    let realm = realm.into();
    let email = email.into();
    let api_key = api_key.into();
    let stream = stream.into();
    let topic = topic.into();

    let client = Client::new();

    let (queue_id, last_event_id) =
        register_event_queue(&client, &realm, &email, &api_key).await?;

    let mut transport = ZulipTransport {
        client,
        realm: realm.clone(),
        email,
        api_key,
        stream: stream.clone(),
        topic: topic.clone(),
        queue_id,
        last_event_id,
        messages: VecDeque::new(),
        seq: 0,
    };

    let initial_snapshot = transport.fetch_initial_snapshot().await?;

    let manifest = Manifest {
        identity: Identity::local(format!("zulip:{realm}/{stream}/{topic}")),
        name: format!("{stream} > {topic}"),
        substrate: None,
        metadata: serde_json::json!({
            "type": "zulip",
            "realm": realm,
            "stream": stream,
            "topic": topic,
        }),
    };

    let conn = ZulipConnection::established(transport, manifest);
    Ok((conn, initial_snapshot))
}
