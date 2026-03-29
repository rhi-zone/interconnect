/// Errors from client operations.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("websocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("connection closed")]
    Closed,

    #[error("codec error: {0}")]
    Codec(#[from] serde_json::Error),

    /// Received an unexpected message type during the auth handshake.
    #[error("handshake error: {0}")]
    Handshake(String),

    /// The server sent a `ServerWire::Error` message.
    #[error("server error {code}: {message}")]
    Server { code: String, message: String },
}
