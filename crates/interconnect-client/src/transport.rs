//! WebSocket transport implementation.

use crate::ClientError;
use futures_util::{SinkExt, StreamExt};
use interconnect_core::Transport;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

/// A WebSocket-backed transport.
pub struct WsTransport {
    inner: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl WsTransport {
    /// Connect to a WebSocket server.
    pub async fn connect(url: &str) -> Result<Self, ClientError> {
        let (ws, _) = connect_async(url).await?;
        Ok(Self { inner: ws })
    }
}

impl Transport for WsTransport {
    type Error = tokio_tungstenite::tungstenite::Error;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let text = String::from_utf8_lossy(data).into_owned();
        self.inner.send(Message::Text(text.into())).await
    }

    async fn recv(&mut self) -> Result<Option<Vec<u8>>, Self::Error> {
        loop {
            match self.inner.next().await {
                None => return Ok(None),
                Some(Err(e)) => return Err(e),
                Some(Ok(Message::Text(t))) => return Ok(Some(t.as_bytes().to_vec())),
                Some(Ok(Message::Binary(b))) => return Ok(Some(b.into())),
                // tungstenite handles Ping/Pong automatically; skip control frames.
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                Some(Ok(Message::Close(_))) => return Ok(None),
                Some(Ok(_)) => continue,
            }
        }
    }
}
