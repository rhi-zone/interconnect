//! Typed connection to an Interconnect authority.

use crate::ClientError;
use interconnect_core::{
    ClientWire, Identity, Manifest, ServerWire, Transport, Wire, from_json, to_json,
};

/// A typed connection to an authority.
///
/// Generic over the transport `T`, intent type `I`, and snapshot type `S`.
/// For multi-authority use, hold two `Connection` instances and `select!`
/// between their `recv()` futures:
///
/// ```ignore
/// tokio::select! {
///     msg = conn_a.recv() => { /* handle room A */ }
///     msg = conn_b.recv() => { /* handle room B */ }
/// }
/// ```
pub struct Connection<T, I, S> {
    transport: T,
    manifest: Manifest,
    _phantom: std::marker::PhantomData<(I, S)>,
}

impl<T, I, S> Connection<T, I, S>
where
    T: Transport,
    T::Error: Into<ClientError>,
    I: Wire,
    S: Wire,
{
    /// Authenticate and establish a live connection.
    ///
    /// Sends `Auth`, waits for `Manifest` then the initial `Snapshot`.
    /// Returns the connection and the initial snapshot.
    pub async fn connect(
        mut transport: T,
        identity: Identity,
        name: Option<String>,
        passport: Option<Vec<u8>>,
    ) -> Result<(Self, S), ClientError> {
        let auth: ClientWire<I> = ClientWire::Auth { identity, name, passport };
        transport.send(&to_json(&auth)?).await.map_err(Into::into)?;

        // Wait for Manifest. Skip System messages (unlikely but possible).
        let manifest = loop {
            let raw = transport.recv().await.map_err(Into::into)?.ok_or(ClientError::Closed)?;
            let msg: ServerWire<S> = from_json(&raw)?;
            match msg {
                ServerWire::Manifest(m) => break m,
                ServerWire::System { .. } => continue,
                ServerWire::Error { code, message } => {
                    return Err(ClientError::Server { code, message });
                }
                other => {
                    return Err(ClientError::Handshake(format!(
                        "expected Manifest, got discriminant {:?}",
                        std::mem::discriminant(&other)
                    )));
                }
            }
        };

        // Wait for initial Snapshot. System broadcasts may arrive first.
        let initial = loop {
            let raw = transport.recv().await.map_err(Into::into)?.ok_or(ClientError::Closed)?;
            let msg: ServerWire<S> = from_json(&raw)?;
            match msg {
                ServerWire::Snapshot { data, .. } => break data,
                ServerWire::System { .. } => continue,
                ServerWire::Error { code, message } => {
                    return Err(ClientError::Server { code, message });
                }
                other => {
                    return Err(ClientError::Handshake(format!(
                        "expected Snapshot, got discriminant {:?}",
                        std::mem::discriminant(&other)
                    )));
                }
            }
        };

        Ok((Self { transport, manifest, _phantom: std::marker::PhantomData }, initial))
    }

    /// Send an intent to the authority.
    pub async fn send_intent(&mut self, intent: I) -> Result<(), ClientError> {
        let msg: ClientWire<I> = ClientWire::Intent(intent);
        self.transport.send(&to_json(&msg)?).await.map_err(Into::into)
    }

    /// Receive the next message from the authority.
    ///
    /// Returns `None` when the connection is closed.
    pub async fn recv(&mut self) -> Result<Option<ServerWire<S>>, ClientError> {
        let raw = match self.transport.recv().await.map_err(Into::into)? {
            Some(b) => b,
            None => return Ok(None),
        };
        Ok(Some(from_json(&raw)?))
    }

    /// Send a ping.
    pub async fn ping(&mut self) -> Result<(), ClientError> {
        let msg: ClientWire<I> = ClientWire::Ping;
        self.transport.send(&to_json(&msg)?).await.map_err(Into::into)
    }

    /// Request transfer to another authority.
    pub async fn request_transfer(&mut self, destination: String) -> Result<(), ClientError> {
        let msg: ClientWire<I> = ClientWire::TransferRequest { destination };
        self.transport.send(&to_json(&msg)?).await.map_err(Into::into)
    }

    /// Create a connection where the platform has already handled authentication.
    ///
    /// Use this for platform connectors (Discord, Slack, etc.) where the
    /// handshake is done natively by the platform SDK rather than via the
    /// Interconnect wire protocol. The caller is responsible for fetching
    /// the initial snapshot separately before constructing the connection.
    pub fn established(transport: T, manifest: Manifest) -> Self {
        Self { transport, manifest, _phantom: std::marker::PhantomData }
    }

    /// The manifest received during the handshake.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
}
