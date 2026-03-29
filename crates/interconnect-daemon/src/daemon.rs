use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, Notify};

use crate::config::{Config, RoomConfig};
use crate::protocol::{Request, Response};

/// Per-room state managed by the daemon.
struct RoomState {
    config: RoomConfig,
    /// All received messages (append-only).
    messages: Vec<serde_json::Value>,
    /// Read cursor: index of the next message not yet delivered to the default reader.
    cursor: usize,
    /// Notified whenever a new message is appended.
    notify: Arc<Notify>,
}

impl RoomState {
    fn new(config: RoomConfig) -> Self {
        Self {
            config,
            messages: Vec::new(),
            cursor: 0,
            notify: Arc::new(Notify::new()),
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

        // Spawn stub message producers for each room.
        // TODO: Replace with real connector instantiation once connectors expose a
        // uniform async connect() interface. Each connector would call push() via
        // a channel or shared state rather than a timer.
        {
            let rooms = Arc::clone(&self.rooms);
            let names: Vec<String> = rooms.lock().await.keys().cloned().collect();
            for name in names {
                let rooms = Arc::clone(&rooms);
                tokio::spawn(stub_producer(rooms, name));
            }
        }

        loop {
            let (stream, _addr) = listener.accept().await?;
            let rooms = Arc::clone(&self.rooms);
            tokio::spawn(handle_connection(stream, rooms));
        }
    }
}

/// Stub: periodically pushes a synthetic message into a room so the daemon can
/// be exercised without real connectors.
///
/// TODO: Remove once connectors are wired in. Replace with a real connector
/// task that drives push() from live data.
async fn stub_producer(rooms: SharedRooms, room_name: String) {
    let mut counter: u64 = 0;
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        counter += 1;
        let msg = serde_json::json!({
            "stub": true,
            "room": room_name,
            "seq": counter,
            "text": format!("stub message {} from {}", counter, room_name),
        });
        let mut guard = rooms.lock().await;
        if let Some(state) = guard.get_mut(&room_name) {
            state.push(msg);
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
            // TODO: Forward to the real connector's send/intent channel once
            // connectors are wired in. Currently echoes the payload back as a
            // received message so the round-trip can be tested end-to-end.
            let mut guard = rooms.lock().await;
            match guard.get_mut(&room) {
                None => Response::error(format!("room not found: {room}")),
                Some(state) => {
                    state.push(serde_json::json!({
                        "echo": true,
                        "payload": payload,
                    }));
                    Response::sent()
                }
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
