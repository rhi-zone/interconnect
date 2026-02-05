//! Game server implementation.

use crate::protocol::{GameIntent, GamePassport, GameSnapshot};
use crate::world::{Player, World};
use futures_util::{SinkExt, StreamExt};
use interconnect_core::Identity;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, RwLock};
use tokio_tungstenite::tungstenite::Message;

use serde::{Deserialize, Serialize};

/// Wire messages for the game.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WireMessage {
    // Client -> Server
    Auth {
        identity: Identity,
        name: String,
        passport: Option<Vec<u8>>,
    },
    Intent(GameIntent),

    // Server -> Client
    Welcome {
        zone_name: String,
        allow_weapons: bool,
    },
    Snapshot(GameSnapshot),
    Transfer {
        destination: String,
        passport: Vec<u8>,
    },
    ImportReport {
        accepted: Vec<String>,
        rejected: Vec<String>,
    },
    Error {
        message: String,
    },
}

type SharedWorld = Arc<RwLock<World>>;

pub async fn run(port: u16, name: String, peer: Option<String>) -> anyhow::Result<()> {
    let world = Arc::new(RwLock::new(World::new(name)));
    let (broadcast_tx, _) = broadcast::channel::<GameSnapshot>(16);

    // Spawn tick loop
    let tick_world = world.clone();
    let tick_broadcast = broadcast_tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(50)); // 20 ticks/sec
        loop {
            interval.tick().await;
            let mut w = tick_world.write().await;
            w.tick();
            let snapshot = w.snapshot();
            let _ = tick_broadcast.send(snapshot);
        }
    });

    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("Listening on ws://{}", addr);

    let peer = Arc::new(peer);

    loop {
        let (stream, client_addr) = listener.accept().await?;
        let world = world.clone();
        let broadcast_tx = broadcast_tx.clone();
        let peer = peer.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, client_addr, world, broadcast_tx, peer).await
            {
                tracing::warn!("Connection error: {}", e);
            }
        });
    }
}

async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    world: SharedWorld,
    broadcast_tx: broadcast::Sender<GameSnapshot>,
    peer: Arc<Option<String>>,
) -> anyhow::Result<()> {
    let ws = tokio_tungstenite::accept_async(stream).await?;
    let (mut sink, mut stream) = ws.split();

    tracing::debug!("New connection from {}", addr);

    // Wait for auth
    let (identity, player_name) = loop {
        let msg = stream
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("Connection closed"))??;
        if let Message::Text(text) = msg {
            let wire: WireMessage = serde_json::from_str(&text)?;
            if let WireMessage::Auth {
                identity,
                name,
                passport,
            } = wire
            {
                let mut w = world.write().await;

                let player = if let Some(passport_data) = passport {
                    if let Some(passport) = GamePassport::from_bytes(&passport_data) {
                        // Apply import policy
                        let import_result = w.apply_import_policy(&passport);

                        // Report what was accepted/rejected
                        let report = WireMessage::ImportReport {
                            accepted: import_result
                                .accepted_items
                                .iter()
                                .map(|i| format!("{:?}", i.kind))
                                .collect(),
                            rejected: import_result
                                .rejected_items
                                .iter()
                                .map(|(i, reason)| format!("{:?}: {}", i.kind, reason))
                                .collect(),
                        };
                        sink.send(Message::Text(serde_json::to_string(&report)?.into()))
                            .await?;

                        tracing::info!(
                            "{} arrived from {}, {} items accepted, {} rejected",
                            passport.name,
                            passport.origin_zone,
                            import_result.accepted_items.len(),
                            import_result.rejected_items.len()
                        );

                        Player::from_passport(passport, import_result)
                    } else {
                        Player::new(identity.clone(), name.clone())
                    }
                } else {
                    Player::new(identity.clone(), name.clone())
                };

                let player_name = player.name.clone();
                w.add_player(player);

                break (identity, player_name);
            }
        }
    };

    // Send welcome
    {
        let w = world.read().await;
        let welcome = WireMessage::Welcome {
            zone_name: w.name.clone(),
            allow_weapons: w.allow_weapons,
        };
        sink.send(Message::Text(serde_json::to_string(&welcome)?.into()))
            .await?;
    }

    // Subscribe to tick broadcasts
    let mut broadcast_rx = broadcast_tx.subscribe();

    // Main loop
    loop {
        tokio::select! {
            // Incoming intent from client
            msg = stream.next() => {
                let msg = match msg {
                    Some(Ok(msg)) => msg,
                    _ => break,
                };

                if let Message::Text(text) = msg {
                    let wire: WireMessage = match serde_json::from_str(&text) {
                        Ok(w) => w,
                        Err(_) => continue,
                    };

                    if let WireMessage::Intent(intent) = wire {
                        handle_intent(&world, &identity, intent, &mut sink, &peer).await?;
                    }
                }
            }

            // Tick snapshot
            snapshot = broadcast_rx.recv() => {
                if let Ok(snapshot) = snapshot {
                    let msg = WireMessage::Snapshot(snapshot);
                    sink.send(Message::Text(serde_json::to_string(&msg)?.into())).await?;
                }
            }
        }
    }

    // Remove player on disconnect
    {
        let mut w = world.write().await;
        w.remove_player(&identity);
    }

    tracing::info!("{} disconnected", player_name);
    Ok(())
}

async fn handle_intent(
    world: &SharedWorld,
    identity: &Identity,
    intent: GameIntent,
    sink: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<TcpStream>,
        Message,
    >,
    peer: &Option<String>,
) -> anyhow::Result<()> {
    match intent {
        GameIntent::Move { dx, dy } => {
            let mut w = world.write().await;
            if let Some(player) = w.players.get_mut(identity) {
                player.move_by(dx, dy);
            }
        }

        GameIntent::PickUp { item_id } => {
            let mut w = world.write().await;
            // Get player position first
            let player_pos = w.players.get(identity).map(|p| (p.x, p.y, p.name.clone()));
            if let Some((px, py, name)) = player_pos
                && let Some(idx) = w.items.iter().position(|i| i.id == item_id)
            {
                let item = &w.items[idx];
                let dist = ((px - item.x).powi(2) + (py - item.y).powi(2)).sqrt();
                if dist < 2.0 {
                    let item = w.items.remove(idx);
                    if let Some(player) = w.players.get_mut(identity) {
                        player.inventory.push(crate::protocol::InventoryItem {
                            kind: item.kind,
                            count: 1,
                        });
                    }
                    tracing::info!("{} picked up {:?}", name, item.kind);
                }
            }
        }

        GameIntent::UseItem { slot } => {
            let mut w = world.write().await;
            if let Some(player) = w.players.get_mut(identity)
                && let Some(item) = player.inventory.get(slot)
                && item.kind == crate::protocol::ItemKind::Potion
            {
                player.health = (player.health + 25).min(player.max_health);
                player.inventory.remove(slot);
                tracing::info!("{} used a potion", player.name);
            }
        }

        GameIntent::Drop { slot } => {
            let mut w = world.write().await;
            // Extract what we need from player first
            let drop_info = w.players.get_mut(identity).and_then(|player| {
                if slot < player.inventory.len() {
                    let item = player.inventory.remove(slot);
                    Some((item.kind, player.x, player.y))
                } else {
                    None
                }
            });
            // Then add to world
            if let Some((kind, x, y)) = drop_info {
                let id = w.tick;
                w.items.push(crate::protocol::WorldItem { id, kind, x, y });
            }
        }

        GameIntent::Transfer { destination } => {
            if peer.as_ref() != Some(&destination) {
                let error = WireMessage::Error {
                    message: format!("Unknown destination: {}", destination),
                };
                sink.send(Message::Text(serde_json::to_string(&error)?.into()))
                    .await?;
                return Ok(());
            }

            let w = world.read().await;
            if let Some(player) = w.players.get(identity) {
                let passport = player.to_passport(w.name.clone());
                let transfer = WireMessage::Transfer {
                    destination,
                    passport: passport.to_bytes(),
                };
                sink.send(Message::Text(serde_json::to_string(&transfer)?.into()))
                    .await?;
                tracing::info!("{} transferring to another zone", player.name);
            }
        }
    }

    Ok(())
}
