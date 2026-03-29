//! Webchat example — a minimal browser-connected Interconnect authority.
//!
//! Serves both HTTP (index.html) and WebSocket (Interconnect room) on port 3030.
//!
//! Usage:
//!   cargo run -p interconnect-example-webchat
//!   Then open http://localhost:3030 in a browser.

use anyhow::Context;
use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::{Html, IntoResponse, Response},
    routing::{any, get},
};
use futures_util::{SinkExt, StreamExt};
use interconnect_core::{Identity, Manifest, from_json_str, to_json_string};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, broadcast};

// ── Protocol types ────────────────────────────────────────────────────────────

/// A single chat message stored in the room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub author: String,
    pub text: String,
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
}

/// The room snapshot broadcast to all clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSnapshot {
    /// Last 100 messages.
    pub messages: Vec<ChatMessage>,
}

/// Intents a client may send.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatIntent {
    SendMessage { text: String },
}

// ── Room state ────────────────────────────────────────────────────────────────

struct RoomState {
    messages: Vec<ChatMessage>,
}

impl RoomState {
    fn new() -> Self {
        Self { messages: Vec::new() }
    }

    fn add_message(&mut self, author: String, text: String) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.messages.push(ChatMessage { author, text, timestamp });
        if self.messages.len() > 100 {
            self.messages.remove(0);
        }
    }

    fn snapshot(&self) -> ChatSnapshot {
        ChatSnapshot { messages: self.messages.clone() }
    }
}

// ── Shared application state ──────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    room: Arc<Mutex<RoomState>>,
    manifest: Manifest,
    broadcast_tx: broadcast::Sender<String>,
    seq: Arc<AtomicU64>,
}

impl AppState {
    fn new() -> Self {
        let (broadcast_tx, _) = broadcast::channel(256);
        let identity = Identity::local("webchat");
        let manifest = Manifest {
            identity,
            name: "WebChat Room".to_string(),
            substrate: None,
            metadata: serde_json::json!({ "type": "webchat" }),
        };
        Self {
            room: Arc::new(Mutex::new(RoomState::new())),
            manifest,
            broadcast_tx,
            seq: Arc::new(AtomicU64::new(1)),
        }
    }

    async fn broadcast_snapshot(&self) {
        let room = self.room.lock().await;
        let seq = self.seq.fetch_add(1, Ordering::SeqCst);
        let wire = interconnect_core::ServerWire::<ChatSnapshot>::Snapshot {
            seq,
            data: room.snapshot(),
        };
        if let Ok(json) = to_json_string(&wire) {
            let _ = self.broadcast_tx.send(json);
        }
    }
}

// ── HTTP handlers ─────────────────────────────────────────────────────────────

async fn index_handler() -> impl IntoResponse {
    Html(include_str!("index.html"))
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(|socket| handle_ws(socket, state))
}

// ── WebSocket / Interconnect room ─────────────────────────────────────────────

async fn handle_ws(socket: WebSocket, state: AppState) {
    if let Err(e) = run_connection(socket, state).await {
        tracing::warn!("Connection error: {e}");
    }
}

async fn run_connection(mut socket: WebSocket, state: AppState) -> anyhow::Result<()> {
    use interconnect_core::ClientWire;

    // ── Auth handshake ────────────────────────────────────────────────────────
    let client_name: String = loop {
        let msg = socket
            .recv()
            .await
            .context("Connection closed during auth")??;

        if let Message::Text(text) = msg {
            let wire: ClientWire<ChatIntent> = match from_json_str(text.as_str()) {
                Ok(w) => w,
                Err(e) => {
                    tracing::warn!("Auth parse error: {e}");
                    continue;
                }
            };
            if let ClientWire::Auth { identity, name, .. } = wire {
                let resolved = name.unwrap_or_else(|| identity.payload().to_string());
                tracing::info!("Client authenticated: {resolved}");
                break resolved;
            }
        }
    };

    // ── Send manifest ─────────────────────────────────────────────────────────
    {
        let wire = interconnect_core::ServerWire::<ChatSnapshot>::Manifest(state.manifest.clone());
        let json = to_json_string(&wire)?;
        socket.send(Message::Text(json.into())).await?;
    }

    // ── Send initial snapshot ─────────────────────────────────────────────────
    {
        let room = state.room.lock().await;
        let wire = interconnect_core::ServerWire::<ChatSnapshot>::Snapshot {
            seq: 0,
            data: room.snapshot(),
        };
        let json = to_json_string(&wire)?;
        drop(room);
        socket.send(Message::Text(json.into())).await?;
    }

    // ── Main loop: receive intents and broadcast updates ──────────────────────
    let mut broadcast_rx = state.broadcast_tx.subscribe();
    let (mut sink, mut stream) = socket.split();

    loop {
        tokio::select! {
            msg = stream.next() => {
                let msg = match msg {
                    Some(Ok(m)) => m,
                    Some(Err(e)) => {
                        tracing::debug!("WebSocket error: {e}");
                        break;
                    }
                    None => break,
                };

                if let Message::Text(text) = msg {
                    let wire: ClientWire<ChatIntent> = match from_json_str(text.as_str()) {
                        Ok(w) => w,
                        Err(e) => {
                            tracing::warn!("Invalid message from {client_name}: {e}");
                            continue;
                        }
                    };

                    match wire {
                        ClientWire::Intent(ChatIntent::SendMessage { text }) => {
                            {
                                let mut room = state.room.lock().await;
                                room.add_message(client_name.clone(), text);
                            }
                            state.broadcast_snapshot().await;
                        }
                        ClientWire::Ping => {
                            let pong = to_json_string(
                                &interconnect_core::ServerWire::<ChatSnapshot>::Pong
                            )?;
                            sink.send(Message::Text(pong.into())).await?;
                        }
                        _ => {}
                    }
                }
            }

            broadcast_msg = broadcast_rx.recv() => {
                match broadcast_msg {
                    Ok(json) => sink.send(Message::Text(json.into())).await?,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("{client_name} missed {n} snapshot(s)");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    tracing::info!("Client disconnected: {client_name}");
    Ok(())
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("webchat=info".parse().unwrap())
                .add_directive("interconnect_example_webchat=info".parse().unwrap()),
        )
        .init();

    let state = AppState::new();

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/ws", any(ws_handler))
        .with_state(state);

    let addr = "127.0.0.1:3030";
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::info!("Listening on http://{addr}");
    tracing::info!("Open http://{addr} in a browser");

    axum::serve(listener, app).await.unwrap();
}
