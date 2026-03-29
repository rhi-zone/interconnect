//! High-level connector entry point.

use crate::transport::WhatsAppTransport;
use crate::types::{WhatsAppError, WhatsAppIntent, WhatsAppSnapshot};
use interconnect_client::Connection;
use interconnect_core::{Identity, Manifest};
use reqwest::Client;

pub type WhatsAppConnection = Connection<WhatsAppTransport, WhatsAppIntent, WhatsAppSnapshot>;

/// Connect to a WhatsApp conversation as an Interconnect room.
///
/// Uses the WhatsApp Business Cloud API. Send intents become API calls;
/// receive is a stub pending webhook integration (see `transport.rs`).
///
/// # Arguments
///
/// - `phone_number_id` — the Meta Business phone number ID (not the phone number
///   itself; found in the Meta Developer Console under WhatsApp > API Setup).
/// - `access_token` — a Meta Graph API access token with `whatsapp_business_messaging`
///   permission.
/// - `recipient_phone` — the recipient's phone number in E.164 format (e.g. `"15551234567"`).
///
/// # Example
///
/// ```ignore
/// use interconnect_connector_whatsapp as whatsapp;
///
/// let (mut conn, snapshot) = whatsapp::connect(
///     "123456789012345",
///     "EAAxxxxxxx...",
///     "15551234567",
/// ).await?;
///
/// conn.send_intent(whatsapp::WhatsAppIntent::SendMessage {
///     text: "hello from another room".to_string(),
/// }).await?;
/// ```
pub async fn connect(
    phone_number_id: impl Into<String>,
    access_token: impl Into<String>,
    recipient_phone: impl Into<String>,
) -> Result<(WhatsAppConnection, WhatsAppSnapshot), WhatsAppError> {
    let phone_number_id = phone_number_id.into();
    let access_token = access_token.into();
    let recipient = recipient_phone.into();

    let http = Client::new();

    // Verify credentials by fetching the phone number's display name.
    let display_name =
        fetch_display_name(&http, &access_token, &phone_number_id).await?;

    // Initial snapshot is empty — inbound messages require webhook setup.
    let snapshot = WhatsAppSnapshot {
        phone_number_id: phone_number_id.clone(),
        recipient: recipient.clone(),
        messages: vec![],
    };

    let transport = WhatsAppTransport {
        http,
        phone_number_id: phone_number_id.clone(),
        access_token,
        recipient: recipient.clone(),
        snapshot: snapshot.clone(),
        seq: 0,
    };

    let manifest = Manifest {
        identity: Identity::local(format!("whatsapp:{phone_number_id}:{recipient}")),
        name: display_name,
        substrate: None,
        metadata: serde_json::json!({
            "type": "whatsapp",
            "phone_number_id": phone_number_id,
            "recipient": recipient,
        }),
    };

    let conn = WhatsAppConnection::established(transport, manifest);
    Ok((conn, snapshot))
}

/// Fetch the WhatsApp Business phone number's display name to use as the room name.
///
/// Calls `GET /v18.0/{phone_number_id}` on the Graph API.
async fn fetch_display_name(
    http: &Client,
    access_token: &str,
    phone_number_id: &str,
) -> Result<String, WhatsAppError> {
    let url = format!("https://graph.facebook.com/v18.0/{phone_number_id}");
    let resp: serde_json::Value = http
        .get(&url)
        .bearer_auth(access_token)
        .query(&[("fields", "display_phone_number,verified_name")])
        .send()
        .await?
        .json()
        .await?;

    if let Some(err) = resp.get("error") {
        let msg = err["message"]
            .as_str()
            .unwrap_or("unknown api error")
            .to_string();
        return Err(WhatsAppError::Api(msg));
    }

    let name = resp["verified_name"]
        .as_str()
        .or_else(|| resp["display_phone_number"].as_str())
        .unwrap_or(phone_number_id)
        .to_string();

    Ok(name)
}
