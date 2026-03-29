//! WhatsApp Business Cloud API transport.
//!
//! Presents a WhatsApp conversation as an Interconnect `Transport`.
//! `ClientWire<WhatsAppIntent>` bytes become Cloud API calls;
//! `recv()` is a stub — the Cloud API is webhook-based for receiving,
//! so full receive support requires an external webhook integration.

use crate::types::{WhatsAppError, WhatsAppIntent, WhatsAppSnapshot};
use interconnect_core::{ClientWire, Transport};
use reqwest::Client;

pub struct WhatsAppTransport {
    pub(crate) http: Client,
    pub(crate) phone_number_id: String,
    pub(crate) access_token: String,
    pub(crate) recipient: String,
    /// Current snapshot; updated when webhook events are delivered.
    /// Not yet consumed by `recv()` — see TODO in that method.
    #[allow(dead_code)]
    pub(crate) snapshot: WhatsAppSnapshot,
    /// Monotonic sequence counter for snapshot frames.
    #[allow(dead_code)]
    pub(crate) seq: u64,
}

impl Transport for WhatsAppTransport {
    type Error = WhatsAppError;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let wire: ClientWire<WhatsAppIntent> = serde_json::from_slice(data)?;
        if let ClientWire::Intent(WhatsAppIntent::SendMessage { text }) = wire {
            let url = format!(
                "https://graph.facebook.com/v18.0/{}/messages",
                self.phone_number_id
            );
            let body = serde_json::json!({
                "messaging_product": "whatsapp",
                "to": self.recipient,
                "type": "text",
                "text": { "body": text },
            });
            let resp: serde_json::Value = self
                .http
                .post(&url)
                .bearer_auth(&self.access_token)
                .json(&body)
                .send()
                .await?
                .json()
                .await?;

            if let Some(errors) = resp.get("error") {
                let msg = errors["message"]
                    .as_str()
                    .unwrap_or("unknown api error")
                    .to_string();
                return Err(WhatsAppError::Api(msg));
            }
        }
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<Vec<u8>>, Self::Error> {
        // TODO: The WhatsApp Business Cloud API delivers inbound messages via webhooks,
        // not polling. Implement receive by standing up a webhook endpoint (e.g. using
        // axum or an existing HTTP server) and feeding events into this transport via a
        // channel. Until then, recv() returns None, making this a send-only connector.
        //
        // For reference, Meta's webhook setup:
        //   https://developers.facebook.com/docs/whatsapp/cloud-api/webhooks
        Ok(None)
    }
}

impl WhatsAppTransport {
    /// Returns the current snapshot; used when webhook events are delivered.
    #[allow(dead_code)]
    pub(crate) fn current_snapshot(&self) -> WhatsAppSnapshot {
        self.snapshot.clone()
    }
}
