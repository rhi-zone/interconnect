//! Type-erased room abstraction.
//!
//! Each connector runs in its own task. The daemon communicates with it via
//! unbounded channels of `serde_json::Value` — intents in, snapshots out.

use interconnect_core::ServerWire;
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::config::RoomConfig;

/// Handle for sending intents to a running connector task.
pub struct RoomHandle {
    /// Send an intent (as JSON) to the connector.
    pub tx: mpsc::UnboundedSender<serde_json::Value>,
}

/// Error returned by `spawn_room`.
#[derive(Debug, thiserror::Error)]
pub enum RoomError {
    #[error("unknown connector: {0}")]
    UnknownConnector(String),
    #[error("missing option '{field}' for connector '{connector}': {source}")]
    BadOptions {
        connector: String,
        field: String,
        source: serde_json::Error,
    },
    #[error("connector error: {0}")]
    Connect(String),
}

/// Spawn a connector task.
///
/// Returns a `RoomHandle` for sending intents. Snapshots received from the
/// connector are pushed onto `push_tx`.
pub async fn spawn_room(
    config: &RoomConfig,
    push_tx: mpsc::UnboundedSender<serde_json::Value>,
) -> Result<RoomHandle, RoomError> {
    let name = config.connector.as_str();
    match name {
        "slack" => spawn_slack(config, push_tx).await,
        "discord" => spawn_discord(config, push_tx).await,
        "sqlite" => spawn_sqlite(config, push_tx).await,
        "telegram" => spawn_telegram(config, push_tx).await,
        "matrix" => spawn_matrix(config, push_tx).await,
        "irc" => spawn_irc(config, push_tx).await,
        "zulip" => spawn_zulip(config, push_tx).await,
        "maillist" => spawn_maillist(config, push_tx).await,
        "signal" => spawn_signal(config, push_tx).await,
        "github" => spawn_github(config, push_tx).await,
        "whatsapp" => spawn_whatsapp(config, push_tx).await,
        "fs" => spawn_fs(config, push_tx).await,
        other => Err(RoomError::UnknownConnector(other.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Options extraction helpers
// ---------------------------------------------------------------------------

fn parse_opts<T: for<'de> Deserialize<'de>>(
    config: &RoomConfig,
) -> Result<T, RoomError> {
    serde_json::from_value(config.options.clone()).map_err(|e| RoomError::BadOptions {
        connector: config.connector.clone(),
        field: String::new(),
        source: e,
    })
}

// ---------------------------------------------------------------------------
// Per-connector spawners
// ---------------------------------------------------------------------------

async fn spawn_slack(
    config: &RoomConfig,
    push_tx: mpsc::UnboundedSender<serde_json::Value>,
) -> Result<RoomHandle, RoomError> {
    #[derive(Deserialize)]
    struct Opts {
        bot_token: String,
        app_token: String,
        channel_id: String,
    }
    let opts: Opts = parse_opts(config)?;

    let (mut conn, snapshot) =
        interconnect_connector_slack::connect(opts.bot_token, opts.app_token, opts.channel_id)
            .await
            .map_err(|e| RoomError::Connect(e.to_string()))?;

    let _ = push_tx.send(
        serde_json::to_value(&snapshot).unwrap_or(serde_json::Value::Null),
    );

    let (intent_tx, mut intent_rx) = mpsc::unbounded_channel::<serde_json::Value>();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = conn.recv() => {
                    match msg {
                        Ok(Some(ServerWire::Snapshot { data, .. })) => {
                            let _ = push_tx.send(
                                serde_json::to_value(&data).unwrap_or(serde_json::Value::Null),
                            );
                        }
                        Ok(Some(_)) => {}
                        Ok(None) => break,
                        Err(e) => { eprintln!("slack room error: {e}"); break; }
                    }
                }
                intent = intent_rx.recv() => {
                    match intent {
                        Some(payload) => {
                            if let Ok(intent) = serde_json::from_value(payload) {
                                let _ = conn.send_intent(intent).await;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });

    Ok(RoomHandle { tx: intent_tx })
}

async fn spawn_discord(
    config: &RoomConfig,
    push_tx: mpsc::UnboundedSender<serde_json::Value>,
) -> Result<RoomHandle, RoomError> {
    #[derive(Deserialize)]
    struct Opts {
        token: String,
        channel_id: u64,
    }
    let opts: Opts = parse_opts(config)?;

    let channel_id = interconnect_connector_discord::Id::new(opts.channel_id);
    let (mut conn, snapshot) =
        interconnect_connector_discord::connect(opts.token, channel_id)
            .await
            .map_err(|e| RoomError::Connect(e.to_string()))?;

    let _ = push_tx.send(
        serde_json::to_value(&snapshot).unwrap_or(serde_json::Value::Null),
    );

    let (intent_tx, mut intent_rx) = mpsc::unbounded_channel::<serde_json::Value>();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = conn.recv() => {
                    match msg {
                        Ok(Some(ServerWire::Snapshot { data, .. })) => {
                            let _ = push_tx.send(
                                serde_json::to_value(&data).unwrap_or(serde_json::Value::Null),
                            );
                        }
                        Ok(Some(_)) => {}
                        Ok(None) => break,
                        Err(e) => { eprintln!("discord room error: {e}"); break; }
                    }
                }
                intent = intent_rx.recv() => {
                    match intent {
                        Some(payload) => {
                            if let Ok(intent) = serde_json::from_value(payload) {
                                let _ = conn.send_intent(intent).await;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });

    Ok(RoomHandle { tx: intent_tx })
}

async fn spawn_sqlite(
    config: &RoomConfig,
    push_tx: mpsc::UnboundedSender<serde_json::Value>,
) -> Result<RoomHandle, RoomError> {
    #[derive(Deserialize)]
    struct Opts {
        path: String,
        table: String,
    }
    let opts: Opts = parse_opts(config)?;

    let (mut conn, snapshot) =
        interconnect_connector_sqlite::connect(opts.path, opts.table)
            .await
            .map_err(|e| RoomError::Connect(e.to_string()))?;

    let _ = push_tx.send(
        serde_json::to_value(&snapshot).unwrap_or(serde_json::Value::Null),
    );

    let (intent_tx, mut intent_rx) = mpsc::unbounded_channel::<serde_json::Value>();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = conn.recv() => {
                    match msg {
                        Ok(Some(ServerWire::Snapshot { data, .. })) => {
                            let _ = push_tx.send(
                                serde_json::to_value(&data).unwrap_or(serde_json::Value::Null),
                            );
                        }
                        Ok(Some(_)) => {}
                        Ok(None) => break,
                        Err(e) => { eprintln!("sqlite room error: {e}"); break; }
                    }
                }
                intent = intent_rx.recv() => {
                    match intent {
                        Some(payload) => {
                            if let Ok(intent) = serde_json::from_value(payload) {
                                let _ = conn.send_intent(intent).await;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });

    Ok(RoomHandle { tx: intent_tx })
}

async fn spawn_telegram(
    config: &RoomConfig,
    push_tx: mpsc::UnboundedSender<serde_json::Value>,
) -> Result<RoomHandle, RoomError> {
    #[derive(Deserialize)]
    struct Opts {
        bot_token: String,
        chat_id: i64,
    }
    let opts: Opts = parse_opts(config)?;

    let (mut conn, snapshot) =
        interconnect_connector_telegram::connect(opts.bot_token, opts.chat_id)
            .await
            .map_err(|e| RoomError::Connect(e.to_string()))?;

    let _ = push_tx.send(
        serde_json::to_value(&snapshot).unwrap_or(serde_json::Value::Null),
    );

    let (intent_tx, mut intent_rx) = mpsc::unbounded_channel::<serde_json::Value>();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = conn.recv() => {
                    match msg {
                        Ok(Some(ServerWire::Snapshot { data, .. })) => {
                            let _ = push_tx.send(
                                serde_json::to_value(&data).unwrap_or(serde_json::Value::Null),
                            );
                        }
                        Ok(Some(_)) => {}
                        Ok(None) => break,
                        Err(e) => { eprintln!("telegram room error: {e}"); break; }
                    }
                }
                intent = intent_rx.recv() => {
                    match intent {
                        Some(payload) => {
                            if let Ok(intent) = serde_json::from_value(payload) {
                                let _ = conn.send_intent(intent).await;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });

    Ok(RoomHandle { tx: intent_tx })
}

async fn spawn_matrix(
    config: &RoomConfig,
    push_tx: mpsc::UnboundedSender<serde_json::Value>,
) -> Result<RoomHandle, RoomError> {
    #[derive(Deserialize)]
    struct Opts {
        homeserver: String,
        access_token: String,
        room_id: String,
    }
    let opts: Opts = parse_opts(config)?;

    let (mut conn, snapshot) =
        interconnect_connector_matrix::connect(opts.homeserver, opts.access_token, opts.room_id)
            .await
            .map_err(|e| RoomError::Connect(e.to_string()))?;

    let _ = push_tx.send(
        serde_json::to_value(&snapshot).unwrap_or(serde_json::Value::Null),
    );

    let (intent_tx, mut intent_rx) = mpsc::unbounded_channel::<serde_json::Value>();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = conn.recv() => {
                    match msg {
                        Ok(Some(ServerWire::Snapshot { data, .. })) => {
                            let _ = push_tx.send(
                                serde_json::to_value(&data).unwrap_or(serde_json::Value::Null),
                            );
                        }
                        Ok(Some(_)) => {}
                        Ok(None) => break,
                        Err(e) => { eprintln!("matrix room error: {e}"); break; }
                    }
                }
                intent = intent_rx.recv() => {
                    match intent {
                        Some(payload) => {
                            if let Ok(intent) = serde_json::from_value(payload) {
                                let _ = conn.send_intent(intent).await;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });

    Ok(RoomHandle { tx: intent_tx })
}

async fn spawn_irc(
    config: &RoomConfig,
    push_tx: mpsc::UnboundedSender<serde_json::Value>,
) -> Result<RoomHandle, RoomError> {
    #[derive(Deserialize)]
    struct Opts {
        server: String,
        port: u16,
        nick: String,
        channel: String,
    }
    let opts: Opts = parse_opts(config)?;

    let (mut conn, snapshot) =
        interconnect_connector_irc::connect(opts.server, opts.port, opts.nick, opts.channel)
            .await
            .map_err(|e| RoomError::Connect(e.to_string()))?;

    let _ = push_tx.send(
        serde_json::to_value(&snapshot).unwrap_or(serde_json::Value::Null),
    );

    let (intent_tx, mut intent_rx) = mpsc::unbounded_channel::<serde_json::Value>();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = conn.recv() => {
                    match msg {
                        Ok(Some(ServerWire::Snapshot { data, .. })) => {
                            let _ = push_tx.send(
                                serde_json::to_value(&data).unwrap_or(serde_json::Value::Null),
                            );
                        }
                        Ok(Some(_)) => {}
                        Ok(None) => break,
                        Err(e) => { eprintln!("irc room error: {e}"); break; }
                    }
                }
                intent = intent_rx.recv() => {
                    match intent {
                        Some(payload) => {
                            if let Ok(intent) = serde_json::from_value(payload) {
                                let _ = conn.send_intent(intent).await;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });

    Ok(RoomHandle { tx: intent_tx })
}

async fn spawn_zulip(
    config: &RoomConfig,
    push_tx: mpsc::UnboundedSender<serde_json::Value>,
) -> Result<RoomHandle, RoomError> {
    #[derive(Deserialize)]
    struct Opts {
        realm: String,
        email: String,
        api_key: String,
        stream: String,
        topic: String,
    }
    let opts: Opts = parse_opts(config)?;

    let (mut conn, snapshot) = interconnect_connector_zulip::connect(
        opts.realm,
        opts.email,
        opts.api_key,
        opts.stream,
        opts.topic,
    )
    .await
    .map_err(|e| RoomError::Connect(e.to_string()))?;

    let _ = push_tx.send(
        serde_json::to_value(&snapshot).unwrap_or(serde_json::Value::Null),
    );

    let (intent_tx, mut intent_rx) = mpsc::unbounded_channel::<serde_json::Value>();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = conn.recv() => {
                    match msg {
                        Ok(Some(ServerWire::Snapshot { data, .. })) => {
                            let _ = push_tx.send(
                                serde_json::to_value(&data).unwrap_or(serde_json::Value::Null),
                            );
                        }
                        Ok(Some(_)) => {}
                        Ok(None) => break,
                        Err(e) => { eprintln!("zulip room error: {e}"); break; }
                    }
                }
                intent = intent_rx.recv() => {
                    match intent {
                        Some(payload) => {
                            if let Ok(intent) = serde_json::from_value(payload) {
                                let _ = conn.send_intent(intent).await;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });

    Ok(RoomHandle { tx: intent_tx })
}

async fn spawn_maillist(
    config: &RoomConfig,
    push_tx: mpsc::UnboundedSender<serde_json::Value>,
) -> Result<RoomHandle, RoomError> {
    #[derive(Deserialize)]
    struct Opts {
        base_url: String,
        username: String,
        password: String,
        list_id: u32,
    }
    let opts: Opts = parse_opts(config)?;

    let (mut conn, snapshot) = interconnect_connector_maillist::connect(
        opts.base_url,
        opts.username,
        opts.password,
        opts.list_id,
    )
    .await
    .map_err(|e| RoomError::Connect(e.to_string()))?;

    let _ = push_tx.send(
        serde_json::to_value(&snapshot).unwrap_or(serde_json::Value::Null),
    );

    let (intent_tx, mut intent_rx) = mpsc::unbounded_channel::<serde_json::Value>();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = conn.recv() => {
                    match msg {
                        Ok(Some(ServerWire::Snapshot { data, .. })) => {
                            let _ = push_tx.send(
                                serde_json::to_value(&data).unwrap_or(serde_json::Value::Null),
                            );
                        }
                        Ok(Some(_)) => {}
                        Ok(None) => break,
                        Err(e) => { eprintln!("maillist room error: {e}"); break; }
                    }
                }
                intent = intent_rx.recv() => {
                    match intent {
                        Some(payload) => {
                            if let Ok(intent) = serde_json::from_value(payload) {
                                let _ = conn.send_intent(intent).await;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });

    Ok(RoomHandle { tx: intent_tx })
}

async fn spawn_signal(
    config: &RoomConfig,
    push_tx: mpsc::UnboundedSender<serde_json::Value>,
) -> Result<RoomHandle, RoomError> {
    #[derive(Deserialize)]
    struct Opts {
        signal_cli_path: String,
        account: String,
        recipient: String,
    }
    let opts: Opts = parse_opts(config)?;

    let (mut conn, snapshot) =
        interconnect_connector_signal::connect(opts.signal_cli_path, opts.account, opts.recipient)
            .await
            .map_err(|e| RoomError::Connect(e.to_string()))?;

    let _ = push_tx.send(
        serde_json::to_value(&snapshot).unwrap_or(serde_json::Value::Null),
    );

    let (intent_tx, mut intent_rx) = mpsc::unbounded_channel::<serde_json::Value>();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = conn.recv() => {
                    match msg {
                        Ok(Some(ServerWire::Snapshot { data, .. })) => {
                            let _ = push_tx.send(
                                serde_json::to_value(&data).unwrap_or(serde_json::Value::Null),
                            );
                        }
                        Ok(Some(_)) => {}
                        Ok(None) => break,
                        Err(e) => { eprintln!("signal room error: {e}"); break; }
                    }
                }
                intent = intent_rx.recv() => {
                    match intent {
                        Some(payload) => {
                            if let Ok(intent) = serde_json::from_value(payload) {
                                let _ = conn.send_intent(intent).await;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });

    Ok(RoomHandle { tx: intent_tx })
}

async fn spawn_github(
    config: &RoomConfig,
    push_tx: mpsc::UnboundedSender<serde_json::Value>,
) -> Result<RoomHandle, RoomError> {
    #[derive(Deserialize)]
    struct Opts {
        token: String,
        owner: String,
        repo: String,
        issue_number: u64,
    }
    let opts: Opts = parse_opts(config)?;

    let (mut conn, snapshot) = interconnect_connector_github::connect(
        opts.token,
        opts.owner,
        opts.repo,
        opts.issue_number,
    )
    .await
    .map_err(|e| RoomError::Connect(e.to_string()))?;

    let _ = push_tx.send(
        serde_json::to_value(&snapshot).unwrap_or(serde_json::Value::Null),
    );

    let (intent_tx, mut intent_rx) = mpsc::unbounded_channel::<serde_json::Value>();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = conn.recv() => {
                    match msg {
                        Ok(Some(ServerWire::Snapshot { data, .. })) => {
                            let _ = push_tx.send(
                                serde_json::to_value(&data).unwrap_or(serde_json::Value::Null),
                            );
                        }
                        Ok(Some(_)) => {}
                        Ok(None) => break,
                        Err(e) => { eprintln!("github room error: {e}"); break; }
                    }
                }
                intent = intent_rx.recv() => {
                    match intent {
                        Some(payload) => {
                            if let Ok(intent) = serde_json::from_value(payload) {
                                let _ = conn.send_intent(intent).await;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });

    Ok(RoomHandle { tx: intent_tx })
}

async fn spawn_whatsapp(
    config: &RoomConfig,
    push_tx: mpsc::UnboundedSender<serde_json::Value>,
) -> Result<RoomHandle, RoomError> {
    #[derive(Deserialize)]
    struct Opts {
        phone_number_id: String,
        access_token: String,
        recipient_phone: String,
    }
    let opts: Opts = parse_opts(config)?;

    let (mut conn, snapshot) = interconnect_connector_whatsapp::connect(
        opts.phone_number_id,
        opts.access_token,
        opts.recipient_phone,
    )
    .await
    .map_err(|e| RoomError::Connect(e.to_string()))?;

    let _ = push_tx.send(
        serde_json::to_value(&snapshot).unwrap_or(serde_json::Value::Null),
    );

    let (intent_tx, mut intent_rx) = mpsc::unbounded_channel::<serde_json::Value>();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = conn.recv() => {
                    match msg {
                        Ok(Some(ServerWire::Snapshot { data, .. })) => {
                            let _ = push_tx.send(
                                serde_json::to_value(&data).unwrap_or(serde_json::Value::Null),
                            );
                        }
                        Ok(Some(_)) => {}
                        Ok(None) => break,
                        Err(e) => { eprintln!("whatsapp room error: {e}"); break; }
                    }
                }
                intent = intent_rx.recv() => {
                    match intent {
                        Some(payload) => {
                            if let Ok(intent) = serde_json::from_value(payload) {
                                let _ = conn.send_intent(intent).await;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });

    Ok(RoomHandle { tx: intent_tx })
}

async fn spawn_fs(
    config: &RoomConfig,
    push_tx: mpsc::UnboundedSender<serde_json::Value>,
) -> Result<RoomHandle, RoomError> {
    #[derive(Deserialize)]
    struct Opts {
        root: String,
    }
    let opts: Opts = parse_opts(config)?;

    let (mut conn, snapshot) = interconnect_connector_fs::connect(opts.root)
        .await
        .map_err(|e| RoomError::Connect(e.to_string()))?;

    let _ = push_tx.send(
        serde_json::to_value(&snapshot).unwrap_or(serde_json::Value::Null),
    );

    let (intent_tx, mut intent_rx) = mpsc::unbounded_channel::<serde_json::Value>();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = conn.recv() => {
                    match msg {
                        Ok(Some(ServerWire::Snapshot { data, .. })) => {
                            let _ = push_tx.send(
                                serde_json::to_value(&data).unwrap_or(serde_json::Value::Null),
                            );
                        }
                        Ok(Some(_)) => {}
                        Ok(None) => break,
                        Err(e) => { eprintln!("fs room error: {e}"); break; }
                    }
                }
                intent = intent_rx.recv() => {
                    match intent {
                        Some(payload) => {
                            if let Ok(intent) = serde_json::from_value(payload) {
                                let _ = conn.send_intent(intent).await;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });

    Ok(RoomHandle { tx: intent_tx })
}
