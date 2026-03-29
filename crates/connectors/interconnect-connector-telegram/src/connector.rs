//! High-level connector entry point.

use crate::transport::TelegramTransport;
use crate::types::{TelegramError, TelegramMessage, TelegramSnapshot};
use interconnect_client::Connection;
use interconnect_core::{Identity, Manifest};
use reqwest::Client;
use std::collections::VecDeque;

pub type TelegramConnection =
    Connection<TelegramTransport, crate::types::TelegramIntent, TelegramSnapshot>;

/// Connect to a Telegram chat as an Interconnect room.
///
/// Uses the Telegram Bot API with long polling (`getUpdates`) to receive
/// messages and `sendMessage` to send them.
///
/// `chat_id` is the numeric Telegram chat ID — negative for groups/channels,
/// positive for private chats.
///
/// Returns a live connection and the initial snapshot (recent message history
/// from the last 24 hours, up to 50 messages).
///
/// # Example
///
/// ```ignore
/// use interconnect_connector_telegram as tg;
///
/// let (mut conn, snapshot) = tg::connect("123456:ABC-DEF", -1001234567890_i64).await?;
///
/// println!("Connected to {}", conn.manifest().name);
/// for msg in &snapshot.messages {
///     println!("{}: {}", msg.from, msg.text);
/// }
///
/// conn.send_intent(tg::TelegramIntent::SendMessage {
///     text: "hello from another room".to_string(),
/// }).await?;
/// ```
pub async fn connect(
    bot_token: impl Into<String>,
    chat_id: i64,
) -> Result<(TelegramConnection, TelegramSnapshot), TelegramError> {
    let bot_token = bot_token.into();
    let http = Client::new();

    // Fetch chat info for the manifest name.
    let chat_title = fetch_chat_title(&http, &bot_token, chat_id).await?;

    // Fetch recent messages from the last 24h as the initial snapshot.
    let initial_messages = fetch_recent_messages(&http, &bot_token, chat_id).await?;

    // Fetch the current update_id offset so we only process new messages going forward.
    let update_offset = fetch_initial_offset(&http, &bot_token).await?;

    let mut messages: VecDeque<TelegramMessage> = VecDeque::new();
    for msg in initial_messages {
        messages.push_back(msg);
        if messages.len() > crate::transport::MAX_MESSAGES {
            messages.pop_front();
        }
    }

    let transport = TelegramTransport {
        http,
        bot_token,
        chat_id,
        chat_title: chat_title.clone(),
        messages: messages.clone(),
        seq: 0,
        update_offset,
    };

    let initial_snapshot = transport.current_snapshot();

    let manifest = Manifest {
        identity: Identity::local(format!("telegram:{chat_id}")),
        name: chat_title,
        substrate: None,
        metadata: serde_json::json!({
            "type": "telegram",
            "chat_id": chat_id,
        }),
    };

    let conn = TelegramConnection::established(transport, manifest);
    Ok((conn, initial_snapshot))
}

fn api_url(bot_token: &str, method: &str) -> String {
    format!("https://api.telegram.org/bot{bot_token}/{method}")
}

/// Fetch the chat title via `getChat`.
async fn fetch_chat_title(
    http: &Client,
    bot_token: &str,
    chat_id: i64,
) -> Result<String, TelegramError> {
    let resp: serde_json::Value = http
        .post(api_url(bot_token, "getChat"))
        .json(&serde_json::json!({ "chat_id": chat_id }))
        .send()
        .await?
        .json()
        .await?;

    if !resp["ok"].as_bool().unwrap_or(false) {
        // Fall back to the numeric ID as a string if getChat fails.
        return Ok(chat_id.to_string());
    }

    let title = resp["result"]["title"]
        .as_str()
        .or_else(|| resp["result"]["first_name"].as_str())
        .unwrap_or(&chat_id.to_string())
        .to_string();

    Ok(title)
}

/// Fetch recent message updates from the last 24 hours to seed the initial snapshot.
///
/// The Bot API only exposes messages via `getUpdates` (no `getHistory`). We do a
/// short non-blocking poll to drain any pending updates, keeping text messages for
/// our chat. This gives a best-effort initial history — bots that have been running
/// continuously will have more context available.
async fn fetch_recent_messages(
    http: &Client,
    bot_token: &str,
    chat_id: i64,
) -> Result<Vec<TelegramMessage>, TelegramError> {
    // Non-blocking poll: timeout=0 returns immediately with buffered updates.
    let resp: serde_json::Value = http
        .post(api_url(bot_token, "getUpdates"))
        .json(&serde_json::json!({
            "timeout": 0,
            "allowed_updates": ["message"],
        }))
        .send()
        .await?
        .json()
        .await?;

    if !resp["ok"].as_bool().unwrap_or(false) {
        // Non-fatal: start with empty history.
        return Ok(Vec::new());
    }

    let updates = match resp["result"].as_array() {
        Some(arr) => arr.clone(),
        None => return Ok(Vec::new()),
    };

    let mut messages: Vec<TelegramMessage> = Vec::new();

    for update in &updates {
        let message = match update.get("message") {
            Some(m) => m,
            None => continue,
        };

        let msg_chat_id = message["chat"]["id"].as_i64().unwrap_or(0);
        if msg_chat_id != chat_id {
            continue;
        }

        let text = match message["text"].as_str() {
            Some(t) => t.to_string(),
            None => continue,
        };

        let message_id = message["message_id"].as_i64().unwrap_or(0) as i32;
        let timestamp = message["date"].as_u64().unwrap_or(0);
        let from = crate::transport::sender_name(message);

        messages.push(TelegramMessage {
            message_id,
            from,
            text,
            timestamp,
        });

        if messages.len() >= crate::transport::MAX_MESSAGES {
            break;
        }
    }

    Ok(messages)
}

/// Query `getUpdates` with `timeout=0` to find the highest update_id seen so far,
/// then return `max_update_id + 1` as the polling offset. This ensures the live
/// transport starts from new updates rather than replaying history.
async fn fetch_initial_offset(
    http: &Client,
    bot_token: &str,
) -> Result<i64, TelegramError> {
    let resp: serde_json::Value = http
        .post(api_url(bot_token, "getUpdates"))
        .json(&serde_json::json!({
            "timeout": 0,
            "allowed_updates": ["message"],
        }))
        .send()
        .await?
        .json()
        .await?;

    if !resp["ok"].as_bool().unwrap_or(false) {
        return Ok(0);
    }

    let updates = match resp["result"].as_array() {
        Some(arr) => arr.clone(),
        None => return Ok(0),
    };

    let max_id = updates
        .iter()
        .filter_map(|u| u["update_id"].as_i64())
        .max()
        .unwrap_or(-1);

    Ok(max_id + 1)
}
