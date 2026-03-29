mod config;
mod daemon;
mod protocol;
mod room;

use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse simple arguments: [--config <path>] [--socket <path>]
    let mut args = std::env::args().skip(1).peekable();
    let mut config_path = PathBuf::from("interconnect.toml");
    let mut socket_path = default_socket_path();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" | "-c" => {
                config_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("--config requires a path"))?,
                );
            }
            "--socket" | "-s" => {
                socket_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("--socket requires a path"))?,
                );
            }
            "--help" | "-h" => {
                eprintln!("Usage: interconnect-daemon [--config <path>] [--socket <path>]");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  --config, -c  Path to interconnect.toml (default: ./interconnect.toml)");
                eprintln!("  --socket, -s  Unix socket path (default: ~/.interconnect/daemon.sock)");
                eprintln!("                Override with INTERCONNECT_SOCK env var");
                std::process::exit(0);
            }
            other => {
                anyhow::bail!("unknown argument: {other}");
            }
        }
    }

    let config = if config_path.exists() {
        config::Config::load(&config_path)?
    } else {
        eprintln!(
            "interconnect-daemon: config file not found at {}, starting with no rooms",
            config_path.display()
        );
        config::Config::default()
    };

    eprintln!(
        "interconnect-daemon: loaded {} room(s) from {}",
        config.room.len(),
        config_path.display()
    );

    let d = daemon::Daemon::new(config, socket_path);
    d.run().await
}

fn default_socket_path() -> PathBuf {
    if let Ok(val) = std::env::var("INTERCONNECT_SOCK") {
        return PathBuf::from(val);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".interconnect").join("daemon.sock")
}
