use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, Notify, mpsc};

use crate::config::{Config, RoomConfig};
use crate::protocol::{Request, Response};
use crate::room::{RoomHandle, spawn_room};

/// Per-room state managed by the daemon.
struct RoomState {
    config: RoomConfig,
    /// All received messages (append-only).
    messages: Vec<serde_json::Value>,
    /// Read cursor: index of the next message not yet delivered to the default reader.
    cursor: usize,
    /// Notified whenever a new message is appended.
    notify: Arc<Notify>,
    /// Handle for sending intents to the connector task. `None` until the
    /// connector has been successfully spawned.
    handle: Option<RoomHandle>,
}

impl RoomState {
    fn new(config: RoomConfig) -> Self {
        Self {
            config,
            messages: Vec::new(),
            cursor: 0,
            notify: Arc::new(Notify::new()),
            handle: None,
        }
    }

    /// Push a message and wake any blocked receivers.
    fn push(&mut self, msg: serde_json::Value) {
        self.messages.push(msg);
        self.notify.notify_waiters();
    }

    /// Return all messages from `cursor` onward and advance the cursor.
    fn drain_pending(&mut self) -> Vec<serde_json::Value> {
        let msgs = self.messages[self.cursor..].to_vec();
        self.cursor = self.messages.len();
        msgs
    }

    /// Current snapshot: metadata + last message if any.
    fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({
            "room": self.config.name,
            "connector": self.config.connector,
            "message_count": self.messages.len(),
            "cursor": self.cursor,
            "last_message": self.messages.last(),
        })
    }
}

type SharedRooms = Arc<Mutex<HashMap<String, RoomState>>>;

pub struct Daemon {
    rooms: SharedRooms,
    socket_path: PathBuf,
}

impl Daemon {
    pub fn new(config: Config, socket_path: PathBuf) -> Self {
        let mut map = HashMap::new();
        for room_cfg in config.room {
            let name = room_cfg.name.clone();
            map.insert(name, RoomState::new(room_cfg));
        }
        Self {
            rooms: Arc::new(Mutex::new(map)),
            socket_path,
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        // Remove stale socket if present.
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path)?;
        }

        // Ensure parent directory exists.
        if let Some(parent) = self.socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let listener = UnixListener::bind(&self.socket_path)?;
        eprintln!(
            "interconnect-daemon: listening on {}",
            self.socket_path.display()
        );

        // Spawn real connector tasks for each configured room.
        {
            let rooms = Arc::clone(&self.rooms);
            let configs: Vec<RoomConfig> = rooms
                .lock()
                .await
                .values()
                .map(|s| s.config.clone())
                .collect();

            for cfg in configs {
                let rooms = Arc::clone(&rooms);
                let room_name = cfg.name.clone();

                // Channel for the connector task to push snapshots back to the daemon.
                let (push_tx, mut push_rx) = mpsc::unbounded_channel::<serde_json::Value>();

                match spawn_room(&cfg, push_tx).await {
                    Ok(handle) => {
                        // Store the handle so Send requests can forward intents.
                        {
                            let mut guard = rooms.lock().await;
                            if let Some(state) = guard.get_mut(&room_name) {
                                state.handle = Some(handle);
                            }
                        }

                        // Drain incoming snapshots from the connector into room state.
                        tokio::spawn(async move {
                            while let Some(msg) = push_rx.recv().await {
                                let mut guard = rooms.lock().await;
                                if let Some(state) = guard.get_mut(&room_name) {
                                    state.push(msg);
                                } else {
                                    break;
                                }
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!(
                            "interconnect-daemon: failed to start connector for room '{room_name}': {e}"
                        );
                    }
                }
            }
        }

        loop {
            let (stream, _addr) = listener.accept().await?;
            let rooms = Arc::clone(&self.rooms);
            tokio::spawn(handle_connection(stream, rooms));
        }
    }
}

async fn handle_connection(stream: UnixStream, rooms: SharedRooms) {
    if let Err(e) = handle_connection_inner(stream, rooms).await {
        eprintln!("interconnect-daemon: connection error: {e}");
    }
}

async fn handle_connection_inner(stream: UnixStream, rooms: SharedRooms) -> std::io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let line = line.trim().to_owned();
        if line.is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Request>(&line) {
            Err(e) => Response::error(format!("invalid request: {e}")),
            Ok(req) => dispatch(req, &rooms).await,
        };

        let mut out = serde_json::to_string(&response).unwrap();
        out.push('\n');
        writer.write_all(out.as_bytes()).await?;
    }

    Ok(())
}

async fn dispatch(req: Request, rooms: &SharedRooms) -> Response {
    match req {
        Request::List => {
            let guard = rooms.lock().await;
            let names: Vec<String> = guard.keys().cloned().collect();
            Response::rooms(names)
        }

        Request::State { room } => {
            let guard = rooms.lock().await;
            match guard.get(&room) {
                None => Response::error(format!("room not found: {room}")),
                Some(state) => Response::state(state.snapshot()),
            }
        }

        Request::Send { room, payload } => {
            let guard = rooms.lock().await;
            match guard.get(&room) {
                None => Response::error(format!("room not found: {room}")),
                Some(state) => match &state.handle {
                    Some(handle) => {
                        let _ = handle.tx.send(payload);
                        Response::sent()
                    }
                    None => Response::error(format!(
                        "room '{room}' has no active connector"
                    )),
                },
            }
        }

        Request::Recv { room, block } => {
            // Fast path: grab what's pending without blocking.
            {
                let mut guard = rooms.lock().await;
                match guard.get_mut(&room) {
                    None => return Response::error(format!("room not found: {room}")),
                    Some(state) => {
                        let msgs = state.drain_pending();
                        if !block || !msgs.is_empty() {
                            return Response::messages(msgs);
                        }
                        // Fall through with notify handle below.
                    }
                }
            }

            // Blocking path: wait for at least one message.
            // We snapshot the notify before releasing the lock so we don't
            // miss a notify that fires between drain and wait.
            let notify = {
                let guard = rooms.lock().await;
                Arc::clone(&guard[&room].notify)
            };

            loop {
                notify.notified().await;
                let mut guard = rooms.lock().await;
                match guard.get_mut(&room) {
                    None => return Response::error(format!("room not found: {room}")),
                    Some(state) => {
                        let msgs = state.drain_pending();
                        if !msgs.is_empty() {
                            return Response::messages(msgs);
                        }
                        // Another waiter drained it; loop and wait again.
                    }
                }
            }
        }
    }
}
