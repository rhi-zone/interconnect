mod cli;
mod config;
mod preset;
mod protocol;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::protocol::{Request, Response};

#[derive(Parser)]
#[command(name = "interconnect", about = "CLI client for the Interconnect daemon")]
struct Cli {
    /// Override the daemon socket path (default: ~/.interconnect/daemon.sock).
    /// Can also be set via INTERCONNECT_SOCK.
    #[arg(long, env = "INTERCONNECT_SOCK")]
    socket: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Receive messages from a room (blocks until a message arrives).
    Recv {
        room: String,
        /// Return immediately even if no messages are pending.
        #[arg(long)]
        nowait: bool,
    },
    /// Send an intent JSON payload to a room.
    Send {
        room: String,
        /// JSON payload to send.
        json: String,
    },
    /// Print the current state snapshot for a room.
    State { room: String },
    /// List all configured rooms.
    List,
    /// Generate configuration hooks for a preset.
    Init {
        /// Preset name (currently only "claude").
        #[arg(long)]
        preset: String,
        /// Write output to this file instead of stdout.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Path to interconnect.toml (default: ./interconnect.toml).
        #[arg(long, default_value = "interconnect.toml")]
        config: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let socket_path = cli
        .socket
        .unwrap_or_else(default_socket_path);

    match cli.command {
        Command::Init {
            preset,
            output,
            config: config_path,
        } => {
            handle_init(&preset, output.as_deref(), &config_path)?;
        }

        Command::List => {
            let req = Request::List;
            let resp = cli::send_request(&socket_path, &req).await?;
            print_response(resp);
        }

        Command::State { room } => {
            let req = Request::State { room };
            let resp = cli::send_request(&socket_path, &req).await?;
            print_response(resp);
        }

        Command::Send { room, json } => {
            let payload: serde_json::Value = serde_json::from_str(&json)
                .map_err(|e| anyhow::anyhow!("invalid JSON payload: {e}"))?;
            let req = Request::Send { room, payload };
            let resp = cli::send_request(&socket_path, &req).await?;
            print_response(resp);
        }

        Command::Recv { room, nowait } => {
            let req = Request::Recv {
                room,
                block: !nowait,
            };
            let resp = cli::send_request(&socket_path, &req).await?;
            print_response(resp);
        }
    }

    Ok(())
}

fn handle_init(
    preset: &str,
    output: Option<&std::path::Path>,
    config_path: &std::path::Path,
) -> anyhow::Result<()> {
    match preset {
        "claude" => {
            let config = if config_path.exists() {
                config::Config::load(config_path)?
            } else {
                eprintln!(
                    "warning: config file not found at {}, generating with no rooms",
                    config_path.display()
                );
                config::Config::default()
            };

            let p = preset::claude::ClaudePreset::from_config(&config);
            let json = serde_json::to_string_pretty(&p.render())?;

            match output {
                None => println!("{json}"),
                Some(path) => {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(path, format!("{json}\n"))?;
                    eprintln!("wrote {}", path.display());
                }
            }
        }
        other => {
            anyhow::bail!("unknown preset: {other} (available: claude)");
        }
    }
    Ok(())
}

/// Pretty-print a daemon response to stdout.
fn print_response(resp: Response) {
    match &resp {
        Response::Messages { messages, .. } => {
            for msg in messages {
                println!("{}", serde_json::to_string_pretty(msg).unwrap());
            }
        }
        Response::Rooms { rooms, .. } => {
            for room in rooms {
                println!("{room}");
            }
        }
        _ => {
            println!("{}", serde_json::to_string_pretty(&resp).unwrap());
        }
    }
}

fn default_socket_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".interconnect").join("daemon.sock")
}
