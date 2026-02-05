//! Chat server implementation.

use crate::protocol::{ChatIntent, ChatMessage, ChatPassport, ChatSnapshot, WireMessage};
use futures_util::{SinkExt, StreamExt};
use interconnect_core::Identity;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, RwLock};
use tokio_tungstenite::tungstenite::Message;

/// Shared server state.
struct ServerState {
    name: String,
    identity: Identity,
    peer: Option<String>,
    messages: Vec<ChatMessage>,
    users: HashMap<Identity, String>, // identity -> display name
}

impl ServerState {
    fn new(name: String, peer: Option<String>) -> Self {
        let identity = Identity::local(&name);
        Self {
            name,
            identity,
            peer,
            messages: Vec::new(),
            users: HashMap::new(),
        }
    }

    fn snapshot(&self) -> ChatSnapshot {
        ChatSnapshot {
            messages: self.messages.iter().rev().take(50).rev().cloned().collect(),
            users: self.users.values().cloned().collect(),
        }
    }

    fn add_message(&mut self, from: &str, text: String) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.messages.push(ChatMessage {
            from: from.to_string(),
            text,
            timestamp,
        });
        // Keep last 100 messages
        if self.messages.len() > 100 {
            self.messages.remove(0);
        }
    }
}

type SharedState = Arc<RwLock<ServerState>>;

pub async fn run(addr: SocketAddr, name: String, peer: Option<String>) -> anyhow::Result<()> {
    let state = Arc::new(RwLock::new(ServerState::new(name, peer)));
    let (broadcast_tx, _) = broadcast::channel::<String>(100);

    let listener = TcpListener::bind(addr).await?;
    tracing::info!("Listening on ws://{}", addr);

    loop {
        let (stream, client_addr) = listener.accept().await?;
        let state = state.clone();
        let broadcast_tx = broadcast_tx.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, client_addr, state, broadcast_tx).await {
                tracing::warn!("Connection error from {}: {}", client_addr, e);
            }
        });
    }
}

async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    state: SharedState,
    broadcast_tx: broadcast::Sender<String>,
) -> anyhow::Result<()> {
    let ws = tokio_tungstenite::accept_async(stream).await?;
    let (mut sink, mut stream) = ws.split();

    tracing::debug!("New connection from {}", addr);

    // Wait for auth
    let (identity, display_name) = loop {
        let msg = stream.next().await.ok_or(anyhow::anyhow!("Connection closed"))??;
        if let Message::Text(text) = msg {
            let wire: WireMessage = serde_json::from_str(&text)?;
            if let WireMessage::Auth { identity, passport } = wire {
                let display_name = if let Some(passport_data) = passport {
                    // Transferring from another server
                    if let Some(passport) = ChatPassport::from_bytes(&passport_data) {
                        tracing::info!("{} arrived from {}", passport.name, passport.origin);
                        passport.name
                    } else {
                        identity.payload().to_string()
                    }
                } else {
                    identity.payload().to_string()
                };
                break (identity, display_name);
            }
        }
    };

    // Send manifest
    {
        let s = state.read().await;
        let manifest = WireMessage::Manifest {
            name: s.name.clone(),
            identity: s.identity.clone(),
        };
        sink.send(Message::Text(serde_json::to_string(&manifest)?.into()))
            .await?;
    }

    // Register user
    {
        let mut s = state.write().await;
        s.users.insert(identity.clone(), display_name.clone());
    }

    // Broadcast join
    let join_msg = format!("{} joined", display_name);
    let _ = broadcast_tx.send(serde_json::to_string(&WireMessage::System { text: join_msg })?);

    // Send initial snapshot
    {
        let s = state.read().await;
        let snapshot = WireMessage::Snapshot(s.snapshot());
        sink.send(Message::Text(serde_json::to_string(&snapshot)?.into()))
            .await?;
    }

    // Subscribe to broadcasts
    let mut broadcast_rx = broadcast_tx.subscribe();

    // Main loop
    loop {
        tokio::select! {
            // Incoming message from client
            msg = stream.next() => {
                let msg = match msg {
                    Some(Ok(msg)) => msg,
                    Some(Err(e)) => {
                        tracing::debug!("WebSocket error: {}", e);
                        break;
                    }
                    None => break,
                };

                if let Message::Text(text) = msg {
                    let wire: WireMessage = match serde_json::from_str(&text) {
                        Ok(w) => w,
                        Err(e) => {
                            tracing::warn!("Invalid message: {}", e);
                            continue;
                        }
                    };

                    match wire {
                        WireMessage::Intent(intent) => {
                            handle_intent(&state, &identity, &display_name, intent, &mut sink, &broadcast_tx).await?;
                        }
                        _ => {
                            tracing::warn!("Unexpected message type");
                        }
                    }
                }
            }

            // Broadcast from another connection
            msg = broadcast_rx.recv() => {
                if let Ok(msg) = msg {
                    sink.send(Message::Text(msg.into())).await?;
                }
            }
        }
    }

    // Unregister user
    {
        let mut s = state.write().await;
        s.users.remove(&identity);
    }

    // Broadcast leave
    let leave_msg = format!("{} left", display_name);
    let _ = broadcast_tx.send(serde_json::to_string(&WireMessage::System { text: leave_msg })?);

    tracing::debug!("Connection closed: {}", addr);
    Ok(())
}

async fn handle_intent(
    state: &SharedState,
    _identity: &Identity,
    display_name: &str,
    intent: ChatIntent,
    sink: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<TcpStream>,
        Message,
    >,
    broadcast_tx: &broadcast::Sender<String>,
) -> anyhow::Result<()> {
    match intent {
        ChatIntent::Message { text } => {
            let mut s = state.write().await;
            s.add_message(display_name, text.clone());

            // Broadcast updated snapshot
            let snapshot = WireMessage::Snapshot(s.snapshot());
            let _ = broadcast_tx.send(serde_json::to_string(&snapshot)?);
        }

        ChatIntent::Transfer { destination } => {
            let s = state.read().await;

            // Check if we know about this peer
            if s.peer.as_ref() != Some(&destination) {
                let error = WireMessage::Error {
                    message: format!("Unknown destination: {}", destination),
                };
                sink.send(Message::Text(serde_json::to_string(&error)?.into()))
                    .await?;
                return Ok(());
            }

            // Create passport
            let passport = ChatPassport::new(display_name.to_string(), s.name.clone());

            // Send transfer
            let transfer = WireMessage::Transfer {
                destination,
                passport: passport.to_bytes(),
            };
            sink.send(Message::Text(serde_json::to_string(&transfer)?.into()))
                .await?;

            tracing::info!("{} transferred to another server", display_name);
        }
    }

    Ok(())
}
