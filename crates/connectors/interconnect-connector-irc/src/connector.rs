//! High-level IRC connector entry point.

use crate::transport::IrcTransport;
use crate::types::{IrcError, IrcIntent, IrcSnapshot};
use interconnect_client::Connection;
use interconnect_core::{Identity, Manifest};
use std::collections::VecDeque;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

pub type IrcConnection = Connection<IrcTransport, IrcIntent, IrcSnapshot>;

/// Connect to an IRC channel as an Interconnect room.
///
/// Opens a plain TCP connection to the server, registers with NICK/USER,
/// waits for the 001 welcome numeric, then JOINs the channel.
///
/// Returns a live connection and an initial (empty) snapshot. Messages
/// accumulate as the transport receives PRIVMSGs.
///
/// # Example
///
/// ```ignore
/// let (mut conn, snapshot) = irc::connect("irc.libera.chat", 6667, "mybot", "#rust").await?;
///
/// println!("Connected to {}", conn.manifest().name);
///
/// conn.send_intent(irc::IrcIntent::SendMessage {
///     text: "hello from another room".to_string(),
/// }).await?;
/// ```
pub async fn connect(
    server: impl Into<String>,
    port: u16,
    nick: impl Into<String>,
    channel: impl Into<String>,
) -> Result<(IrcConnection, IrcSnapshot), IrcError> {
    let server = server.into();
    let nick = nick.into();
    let channel = channel.into();

    let stream = TcpStream::connect((&*server, port)).await?;
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half).lines();

    // Register: send NICK and USER before waiting for welcome.
    write_half
        .write_all(format!("NICK {nick}\r\n").as_bytes())
        .await?;
    write_half
        .write_all(
            format!("USER {nick} 0 * :Interconnect IRC Connector\r\n").as_bytes(),
        )
        .await?;

    // Wait for 001 (RPL_WELCOME), handling PINGs during registration.
    loop {
        let line = match reader.next_line().await? {
            Some(l) => l,
            None => {
                return Err(IrcError::Protocol(
                    "connection closed before welcome".to_string(),
                ))
            }
        };
        let line = line.trim_end_matches('\r').to_string();

        if let Some(rest) = line.strip_prefix("PING ") {
            write_half
                .write_all(format!("PONG {rest}\r\n").as_bytes())
                .await?;
            continue;
        }

        // IRC message: ":server 001 nick :Welcome..."
        // Check for numeric 001 anywhere in the line.
        let parts: Vec<&str> = line.splitn(4, ' ').collect();
        if parts.len() >= 2 && parts[1] == "001" {
            break;
        }
    }

    // Join the channel.
    write_half
        .write_all(format!("JOIN {channel}\r\n").as_bytes())
        .await?;

    let transport = IrcTransport {
        lines: reader,
        writer: write_half,
        channel: channel.clone(),
        server: server.clone(),
        messages: VecDeque::new(),
        seq: 0,
    };

    let initial_snapshot = transport.current_snapshot();

    let manifest = Manifest {
        identity: Identity::local(format!("irc:{server}{channel}")),
        name: channel.clone(),
        substrate: None,
        metadata: serde_json::json!({
            "type": "irc",
            "server": server,
            "channel": channel,
        }),
    };

    let conn = IrcConnection::established(transport, manifest);
    Ok((conn, initial_snapshot))
}
