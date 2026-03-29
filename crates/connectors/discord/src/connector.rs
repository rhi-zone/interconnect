//! High-level connector entry point.

use crate::transport::{DiscordTransport, make_shard};
use crate::types::{DiscordError, DiscordIntent, DiscordSnapshot};
use interconnect_client::Connection;
use interconnect_core::{Identity, Manifest};
use std::collections::VecDeque;
use std::sync::Arc;
use twilight_http::Client;
use twilight_model::id::{Id, marker::ChannelMarker};

pub type DiscordConnection = Connection<DiscordTransport, DiscordIntent, DiscordSnapshot>;

/// Connect to a Discord channel as an Interconnect room.
///
/// Returns a live connection and the initial snapshot (recent message history).
///
/// # Example
///
/// ```ignore
/// let (mut conn, snapshot) = discord::connect(token, channel_id).await?;
///
/// loop {
///     tokio::select! {
///         msg = conn.recv() => { /* handle Discord events */ }
///         msg = other_conn.recv() => { /* relay to Discord */ }
///     }
/// }
/// ```
pub async fn connect(
    token: impl Into<String>,
    channel_id: Id<ChannelMarker>,
) -> Result<(DiscordConnection, DiscordSnapshot), DiscordError> {
    let token = token.into();
    let http = Arc::new(Client::new(token.clone()));

    // Fetch channel metadata for the manifest.
    let channel = http.channel(channel_id).await?.model().await?;
    let channel_name = channel
        .name
        .clone()
        .unwrap_or_else(|| channel_id.to_string());

    let shard = make_shard(token);

    let mut transport = DiscordTransport {
        shard,
        http,
        channel_id,
        channel_name: channel_name.clone(),
        messages: VecDeque::new(),
        seq: 0,
    };

    let initial_snapshot = transport.fetch_initial_snapshot().await?;

    let manifest = Manifest {
        identity: Identity::local(format!("discord:{channel_id}")),
        name: channel_name,
        substrate: None,
        metadata: serde_json::json!({
            "type": "discord",
            "channel_id": channel_id.to_string(),
        }),
    };

    let conn = DiscordConnection::established(transport, manifest);
    Ok((conn, initial_snapshot))
}
