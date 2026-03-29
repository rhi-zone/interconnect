/// Unix socket protocol — newline-delimited JSON.
///
/// Requests sent from CLI to daemon, responses from daemon back to CLI.
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    /// Receive messages from a room.
    Recv {
        room: String,
        /// If true, block until at least one message is available.
        #[serde(default)]
        block: bool,
    },
    /// Send an intent payload to a room.
    Send {
        room: String,
        payload: serde_json::Value,
    },
    /// Get the current snapshot/state for a room.
    State { room: String },
    /// List all configured rooms.
    List,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Response {
    Messages {
        ok: bool,
        messages: Vec<serde_json::Value>,
    },
    Sent {
        ok: bool,
    },
    State {
        ok: bool,
        snapshot: serde_json::Value,
    },
    Rooms {
        ok: bool,
        rooms: Vec<String>,
    },
    Error {
        ok: bool,
        error: String,
    },
}

#[allow(dead_code)]
impl Response {
    pub fn error(msg: impl Into<String>) -> Self {
        Response::Error {
            ok: false,
            error: msg.into(),
        }
    }

    pub fn messages(msgs: Vec<serde_json::Value>) -> Self {
        Response::Messages {
            ok: true,
            messages: msgs,
        }
    }

    pub fn sent() -> Self {
        Response::Sent { ok: true }
    }

    pub fn state(snapshot: serde_json::Value) -> Self {
        Response::State {
            ok: true,
            snapshot,
        }
    }

    pub fn rooms(names: Vec<String>) -> Self {
        Response::Rooms { ok: true, rooms: names }
    }
}
