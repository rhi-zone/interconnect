//! Interactive terminal client for a process room.
//!
//! Connects to a process room server, prints output as it arrives, and
//! forwards terminal input as `SendInput` intents.
//!
//! Usage:
//!   cargo run --bin process-client -- ws://localhost:8080 --name alice

mod protocol;

use interconnect_client::WsConnection;
use interconnect_core::{Identity, ServerWire};
use protocol::{ProcessIntent, ProcessSnapshot};
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args: Vec<String> = std::env::args().collect();

    let url = args.get(1).map(|s| s.as_str()).unwrap_or("ws://localhost:8080");
    let name = parse_flag_str(&args, "--name").unwrap_or_else(|| "client".to_string());

    eprintln!("Connecting to {} as {}...", url, name);

    let transport = interconnect_client::WsTransport::connect(url).await?;
    let (mut conn, snapshot): (WsConnection<ProcessIntent, ProcessSnapshot>, ProcessSnapshot) =
        WsConnection::connect(transport, Identity::local(&name), Some(name), None).await?;

    eprintln!(
        "Connected to '{}' — command: {}",
        conn.manifest().name,
        snapshot.command
    );

    // Print the initial snapshot
    for line in &snapshot.lines {
        println!("{}", line);
    }
    if !snapshot.running {
        eprintln!("[process exited with {:?}]", snapshot.exit_code);
    }

    // Track how many lines we've seen so we only print new ones each snapshot.
    let mut seen = snapshot.lines.len();

    // Concurrent: stdin → intents, snapshots → stdout.
    let mut stdin = BufReader::new(tokio::io::stdin()).lines();

    loop {
        tokio::select! {
            // New line from terminal → send as intent
            line = stdin.next_line() => {
                match line? {
                    None => break, // EOF
                    Some(text) => {
                        conn.send_intent(ProcessIntent::SendInput { text }).await?;
                    }
                }
            }

            // New message from server → print new output lines
            msg = conn.recv() => {
                match msg? {
                    None => {
                        eprintln!("[disconnected]");
                        break;
                    }
                    Some(ServerWire::Snapshot { data, .. }) => {
                        // Print only the lines we haven't seen yet.
                        for line in data.lines.iter().skip(seen) {
                            println!("{}", line);
                        }
                        seen = data.lines.len();

                        if !data.running && seen == data.lines.len() {
                            eprintln!("[process exited with {:?}]", data.exit_code);
                        }
                    }
                    Some(ServerWire::Error { code, message }) => {
                        eprintln!("[error {code}]: {message}");
                    }
                    Some(ServerWire::System { message }) => {
                        eprintln!("[system]: {message}");
                    }
                    Some(_) => {}
                }
            }
        }
    }

    Ok(())
}

fn parse_flag_str(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}
