//! Federated microblog example.
//!
//! Demonstrates Interconnect for async/HTTP use cases:
//! - Posts live on their origin server (authoritative)
//! - Other servers can fetch posts (no replication)
//! - Users can follow users on other servers
//!
//! Run:
//!   cargo run -p interconnect-example-microblog -- --port 8001 --name "alice"
//!   cargo run -p interconnect-example-microblog -- --port 8002 --name "bob"
//!
//! Then:
//!   curl -X POST localhost:8001/post -d '{"text":"Hello from alice!"}'
//!   curl localhost:8001/timeline
//!   curl localhost:8001/feed/localhost:8002/bob  # fetch bob's posts from alice's server

mod protocol;
mod server;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("microblog=info".parse()?),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let port = parse_arg(&args, "--port").unwrap_or(8001);
    let name = parse_arg_string(&args, "--name").unwrap_or_else(|| "user".to_string());

    tracing::info!("Starting @{}@localhost:{}", name, port);

    server::run(port, name).await
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
