//! Federated forum example.
//!
//! Demonstrates Interconnect for discussion boards:
//! - Threads and replies (hierarchical content)
//! - Pagination
//! - Cross-server identity for posting
//! - Import policy for reputation
//!
//! Run:
//!   cargo run -p interconnect-example-forum -- --port 8001 --name "Tech Forum"
//!   cargo run -p interconnect-example-forum -- --port 8002 --name "Gaming Forum"
//!
//! Then:
//!   curl localhost:8001/threads
//!   curl -X POST localhost:8001/threads -d '{"title":"Hello","body":"First post!"}'
//!   curl localhost:8001/threads/1

mod protocol;
mod server;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("forum=info".parse()?))
        .init();

    let args: Vec<String> = std::env::args().collect();
    let port = parse_arg(&args, "--port").unwrap_or(8001);
    let name = parse_arg_string(&args, "--name").unwrap_or_else(|| "Forum".to_string());

    tracing::info!("Starting '{}' on port {}", name, port);

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
