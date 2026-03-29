//! Transport abstraction for the Interconnect protocol.
//!
//! A transport moves bytes between a client and an authority.
//! The protocol layer speaks messages; the transport layer moves bytes.
//!
//! Implementations: WebSocket, Unix socket, HTTP long-poll, message queue.
//!
//! Note: Discord is NOT a transport. It is a separate authority with its own
//! rooms. A client can be connected to Discord and another authority
//! simultaneously; those are two rooms, not one room over a Discord transport.

/// A byte-level transport channel.
///
/// Transports carry raw message bytes in each direction. The protocol layer
/// sits above this, framing those bytes into `ClientWire`/`ServerWire` messages.
pub trait Transport: Send {
    /// Transport-specific error type.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Send a message.
    fn send(
        &mut self,
        data: &[u8],
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send;

    /// Receive the next message. Returns `None` when the connection is closed.
    fn recv(
        &mut self,
    ) -> impl std::future::Future<Output = Result<Option<Vec<u8>>, Self::Error>> + Send;
}
