//! High-level connector entry point.

use crate::transport::SlackTransport;
use crate::types::{SlackError, SlackIntent, SlackMessage, SlackSnapshot};
use interconnect_client::Connection;
use interconnect_core::{Identity, Manifest};
use reqwest::Client;
use std::collections::VecDeque;
use tokio_tungstenite::connect_async;

pub type SlackConnection = Connection<SlackTransport, SlackIntent, SlackSnapshot>;

/// Connect to a Slack channel as an Interconnect room.
///
/// Uses Socket Mode for receiving events (no public endpoint required) and
/// the Web API for sending messages.
///
/// Returns a live connection and the initial snapshot (recent message history).
///
/// # Example
///
/// ```ignore
/// let (mut conn, snapshot) = slack::connect(bot_token, app_token, channel_id).await?;
///
/// println!("Connected to #{}", conn.manifest().name);
/// for msg in &snapshot.messages {
///     println!("{}: {}", msg.user_name, msg.text);
/// }
///
/// conn.send_intent(slack::SlackIntent::SendMessage {
///     text: "hello from another room".to_string(),
/// }).await?;
/// ```
pub async fn connect(
    bot_token: impl Into<String>,
    app_token: impl Into<String>,
    channel_id: impl Into<String>,
) -> Result<(SlackConnection, SlackSnapshot), SlackError> {
    let bot_token = bot_token.into();
    let app_token = app_token.into();
    let channel_id = channel_id.into();

    let http = Client::new();

    // Fetch the WebSocket URL for Socket Mode.
    let ws_url = open_socket_mode_connection(&http, &app_token).await?;

    // Fetch channel info for the manifest name.
    let channel_name = fetch_channel_name(&http, &bot_token, &channel_id).await?;

    // Connect to Socket Mode WebSocket.
    let (ws, _) = connect_async(&ws_url).await?;

    let mut transport = SlackTransport {
        ws,
        http: http.clone(),
        bot_token: bot_token.clone(),
        channel_id: channel_id.clone(),
        channel_name: channel_name.clone(),
        messages: VecDeque::new(),
        seq: 0,
    };

    // Fetch initial message history via conversations.history.
    let initial_snapshot = transport
        .fetch_initial_snapshot(&http, &bot_token, &channel_id)
        .await?;

    let manifest = Manifest {
        identity: Identity::local(format!("slack:{channel_id}")),
        name: channel_name,
        substrate: None,
        metadata: serde_json::json!({
            "type": "slack",
            "channel_id": channel_id,
        }),
    };

    let conn = SlackConnection::established(transport, manifest);
    Ok((conn, initial_snapshot))
}

/// Call `apps.connections.open` to get the Socket Mode WebSocket URL.
async fn open_socket_mode_connection(
    http: &Client,
    app_token: &str,
) -> Result<String, SlackError> {
    let resp: serde_json::Value = http
        .post("https://slack.com/api/apps.connections.open")
        .bearer_auth(app_token)
        .send()
        .await?
        .json()
        .await?;

    if !resp["ok"].as_bool().unwrap_or(false) {
        let err = resp["error"].as_str().unwrap_or("unknown").to_string();
        return Err(SlackError::Api(err));
    }

    let url = resp["url"]
        .as_str()
        .ok_or_else(|| SlackError::Api("missing url in apps.connections.open response".into()))?
        .to_string();

    Ok(url)
}

/// Fetch the channel name via `conversations.info`.
async fn fetch_channel_name(
    http: &Client,
    bot_token: &str,
    channel_id: &str,
) -> Result<String, SlackError> {
    let resp: serde_json::Value = http
        .get("https://slack.com/api/conversations.info")
        .bearer_auth(bot_token)
        .query(&[("channel", channel_id)])
        .send()
        .await?
        .json()
        .await?;

    if !resp["ok"].as_bool().unwrap_or(false) {
        // Fall back to channel_id if info fetch fails (e.g. missing scope).
        return Ok(channel_id.to_string());
    }

    let name = resp["channel"]["name"]
        .as_str()
        .unwrap_or(channel_id)
        .to_string();

    Ok(name)
}

impl SlackTransport {
    /// Fetch recent messages via `conversations.history` and populate the buffer.
    pub(crate) async fn fetch_initial_snapshot(
        &mut self,
        http: &Client,
        bot_token: &str,
        channel_id: &str,
    ) -> Result<SlackSnapshot, SlackError> {
        let resp: serde_json::Value = http
            .get("https://slack.com/api/conversations.history")
            .bearer_auth(bot_token)
            .query(&[("channel", channel_id), ("limit", "50")])
            .send()
            .await?
            .json()
            .await?;

        if !resp["ok"].as_bool().unwrap_or(false) {
            let err = resp["error"].as_str().unwrap_or("unknown").to_string();
            return Err(SlackError::Api(err));
        }

        let messages = resp["messages"].as_array().cloned().unwrap_or_default();

        // API returns newest-first; collect into oldest-first order.
        let mut parsed: Vec<SlackMessage> = Vec::with_capacity(messages.len());
        for msg in &messages {
            // Skip non-plain messages (subtypes like channel_join, etc.).
            if msg.get("subtype").is_some() {
                continue;
            }
            let ts = msg["ts"].as_str().unwrap_or("0").to_string();
            let user_id = msg["user"].as_str().unwrap_or("").to_string();
            let text = msg["text"].as_str().unwrap_or("").to_string();
            let timestamp = SlackTransport::parse_ts(&ts);

            let user_name = if !user_id.is_empty() {
                self.fetch_user_name(&user_id).await.unwrap_or_else(|_| user_id.clone())
            } else {
                user_id.clone()
            };

            parsed.push(SlackMessage {
                ts,
                user_id,
                user_name,
                text,
                timestamp,
            });
        }

        // Reverse to oldest-first.
        parsed.reverse();

        for msg in parsed {
            self.messages.push_back(msg);
        }

        Ok(self.current_snapshot())
    }
}
