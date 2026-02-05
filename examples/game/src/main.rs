//! Federated game example.
//!
//! Demonstrates Interconnect for real-time games:
//! - Tick-based snapshots (20 ticks/sec)
//! - Player positions, physics
//! - Rich passport (inventory, stats)
//! - Import policy (destination decides what items to accept)
//!
//! Run two "zones":
//!   cargo run -p interconnect-example-game -- --port 8001 --name "Forest" --peer ws://localhost:8002
//!   cargo run -p interconnect-example-game -- --port 8002 --name "Cave" --peer ws://localhost:8001
//!
//! The Cave zone has a stricter import policy (no weapons allowed).

mod protocol;
mod server;
mod world;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("game=info".parse()?))
        .init();

    let args: Vec<String> = std::env::args().collect();
    let port = parse_arg(&args, "--port").unwrap_or(8001);
    let name = parse_arg_string(&args, "--name").unwrap_or_else(|| "Zone".to_string());
    let peer = parse_arg_string(&args, "--peer");

    tracing::info!("Starting zone '{}' on port {}", name, port);

    server::run(port, name, peer).await
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
