//! Process room example.
//!
//! Makes a running subprocess into an Interconnect room. Clients connect via
//! WebSocket, send text to the process's stdin, and receive its stdout/stderr
//! as snapshots.
//!
//! Usage:
//!   cargo run --example process -- --port 8080 --name "my-room" -- bash
//!   cargo run --example process -- -- python3 -i
//!   cargo run --example process -- -- cat

mod authority;
mod protocol;
mod server;

use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("process=info".parse()?),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();

    let port = parse_flag_u16(&args, "--port").unwrap_or(8080);
    let name = parse_flag_str(&args, "--name")
        .unwrap_or_else(|| format!("process:{port}"));

    // Everything after `--` is the command to run.
    let command_args: Vec<String> =
        if let Some(pos) = args.iter().position(|a| a == "--") {
            args[pos + 1..].to_vec()
        } else {
            vec!["bash".to_string()]
        };

    if command_args.is_empty() {
        anyhow::bail!("Usage: process [--port N] [--name NAME] -- <command> [args...]");
    }

    let command = command_args[0].clone();
    let cmd_args = command_args[1..].to_vec();

    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    tracing::info!("Starting process room '{name}' on {addr}");
    tracing::info!("Command: {command} {}", cmd_args.join(" "));

    server::run(addr, name, command, cmd_args).await
}

fn parse_flag_u16(args: &[String], flag: &str) -> Option<u16> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
}

fn parse_flag_str(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}
