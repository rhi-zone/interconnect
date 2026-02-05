//! Federated chat example.
//!
//! Demonstrates Interconnect concepts with a simple chat server:
//! - Multiple servers can run independently
//! - Users can transfer between servers
//! - Messages stay on origin server (no replication)
//!
//! Run two servers:
//!   cargo run --example chat -- --port 8001 --name "Server A" --peer ws://localhost:8002
//!   cargo run --example chat -- --port 8002 --name "Server B" --peer ws://localhost:8001

mod protocol;
mod server;

use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("chat=info".parse()?))
        .init();

    let args: Vec<String> = std::env::args().collect();
    let port = parse_arg(&args, "--port").unwrap_or(8001);
    let name = parse_arg_string(&args, "--name").unwrap_or_else(|| format!("Server:{port}"));
    let peer = parse_arg_string(&args, "--peer");

    let addr: SocketAddr = ([127, 0, 0, 1], port).into();

    tracing::info!("Starting {} on {}", name, addr);
    if let Some(ref p) = peer {
        tracing::info!("Peer server: {}", p);
    }

    server::run(addr, name, peer).await
}

fn parse_arg(args: &[String], flag: &str) -> Option<u16> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
}

fn parse_arg_string(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}
