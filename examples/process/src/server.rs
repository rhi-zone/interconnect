//! WebSocket server for a process room.

use crate::authority::ProcessAuthority;
use crate::protocol::{ProcessIntent, ProcessSnapshot};
use futures_util::{SinkExt, StreamExt};
use interconnect_core::{
    from_json_str, to_json_string, ClientWire, Identity, Manifest, ServerWire, Session,
    SimpleAuthority,
};
use std::net::SocketAddr;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, broadcast, mpsc};
use tokio_tungstenite::tungstenite::Message;

struct ServerState {
    authority: ProcessAuthority,
    manifest: Manifest,
    next_session_id: u64,
}

type SharedState = Arc<RwLock<ServerState>>;

pub async fn run(
    addr: SocketAddr,
    name: String,
    command: String,
    args: Vec<String>,
) -> anyhow::Result<()> {
    let identity = Identity::local(&name);
    let manifest = Manifest {
        identity,
        name: name.clone(),
        substrate: None,
        metadata: serde_json::json!({ "type": "process", "command": &command }),
    };

    let (update_tx, mut update_rx) = mpsc::unbounded_channel::<()>();
    let (broadcast_tx, _) = broadcast::channel::<String>(256);

    let authority = ProcessAuthority::spawn(&command, &args, update_tx).await?;
    tracing::info!("Process started: {command}");

    let state = Arc::new(RwLock::new(ServerState {
        authority,
        manifest,
        next_session_id: 1,
    }));

    let listener = TcpListener::bind(addr).await?;
    tracing::info!("Listening on ws://{addr}");

    // Push a new snapshot to all clients whenever the process produces output.
    let seq_counter = Arc::new(AtomicU64::new(1));
    {
        let state = state.clone();
        let broadcast_tx = broadcast_tx.clone();
        let seq_counter = seq_counter.clone();
        tokio::spawn(async move {
            while update_rx.recv().await.is_some() {
                let s = state.read().await;
                let snapshot = s.authority.snapshot();
                let seq = seq_counter.fetch_add(1, Ordering::SeqCst);
                if let Ok(msg) =
                    to_json_string(&ServerWire::<ProcessSnapshot>::Snapshot { seq, data: snapshot })
                {
                    let _ = broadcast_tx.send(msg);
                }
            }
        });
    }

    loop {
        let (stream, client_addr) = listener.accept().await?;
        let state = state.clone();
        let broadcast_tx = broadcast_tx.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, client_addr, state, broadcast_tx).await {
                tracing::warn!("Connection error from {client_addr}: {e}");
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

    tracing::debug!("Connection from {addr}");

    // Auth handshake
    let session = loop {
        let msg = stream
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("Connection closed during auth"))??;

        if let Message::Text(text) = msg {
            let wire: ClientWire<ProcessIntent> = from_json_str(&text)?;
            if let ClientWire::Auth { identity, name, .. } = wire {
                let mut s = state.write().await;
                let id = s.next_session_id;
                s.next_session_id += 1;
                let display_name = name.unwrap_or_else(|| identity.payload().to_string());
                let session = Session::new(id, identity, display_name);
                s.authority.on_connect(&session)?;
                break session;
            }
        }
    };

    // Send manifest
    {
        let s = state.read().await;
        let msg: ServerWire<ProcessSnapshot> = ServerWire::Manifest(s.manifest.clone());
        sink.send(Message::Text(to_json_string(&msg)?.into())).await?;
    }

    // Send initial snapshot
    {
        let s = state.read().await;
        let snapshot = s.authority.snapshot();
        let msg: ServerWire<ProcessSnapshot> = ServerWire::Snapshot { seq: 0, data: snapshot };
        sink.send(Message::Text(to_json_string(&msg)?.into())).await?;
    }

    let mut broadcast_rx = broadcast_tx.subscribe();

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
                    let wire: ClientWire<ProcessIntent> = match from_json_str(&text) {
                        Ok(w) => w,
                        Err(e) => {
                            tracing::warn!("Invalid message: {e}");
                            continue;
                        }
                    };

                    match wire {
                        ClientWire::Intent(intent) => {
                            let mut s = state.write().await;
                            if let Err(e) = s.authority.handle_intent(&session, intent) {
                                let msg: ServerWire<ProcessSnapshot> =
                                    ServerWire::error("intent_error", e.to_string());
                                sink.send(Message::Text(to_json_string(&msg)?.into())).await?;
                            }
                        }
                        ClientWire::Ping => {
                            sink.send(Message::Text(
                                to_json_string(&ServerWire::<ProcessSnapshot>::Pong)?.into(),
                            ))
                            .await?;
                        }
                        _ => {}
                    }
                }
            }

            msg = broadcast_rx.recv() => {
                match msg {
                    Ok(m) => sink.send(Message::Text(m.into())).await?,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("{} missed {n} snapshot(s)", session.name);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    {
        let mut s = state.write().await;
        s.authority.on_disconnect(&session);
    }

    tracing::debug!("Connection closed: {addr}");
    Ok(())
}
