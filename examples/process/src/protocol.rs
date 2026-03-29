//! Protocol types for a process room.
//!
//! A process room wraps a running subprocess. Clients send input to its stdin
//! and receive its stdout/stderr as snapshots.

use serde::{Deserialize, Serialize};

/// Intent sent by a client to steer the process.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProcessIntent {
    /// Send a line of text to the process's stdin.
    SendInput { text: String },
    /// Send a signal to the process.
    SendSignal { signal: ProcessSignal },
}

/// Signals a client can request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessSignal {
    /// SIGINT (Ctrl-C).
    Interrupt,
    /// SIGTERM.
    Terminate,
}

/// Snapshot of the process room state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSnapshot {
    /// Recent output lines (stdout + stderr interleaved, last 200).
    pub lines: Vec<String>,
    /// Whether the process is still running.
    pub running: bool,
    /// Exit code once the process has exited.
    pub exit_code: Option<i32>,
    /// The command being run (display only).
    pub command: String,
}

/// Passport for clients transferring between process rooms.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)] // constructed by the server authority, not client-side
pub struct ProcessPassport {
    pub name: String,
    pub origin: String,
}
