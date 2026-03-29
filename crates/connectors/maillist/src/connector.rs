//! High-level connector entry point.

use crate::transport::MailTransport;
use crate::types::{MailError, MailIntent, MailSnapshot};
use interconnect_client::Connection;
use interconnect_core::{Identity, Manifest};
use reqwest::Client;

pub type MailConnection = Connection<MailTransport, MailIntent, MailSnapshot>;

/// Connect to a Listmonk mailing list as an Interconnect room.
///
/// Returns a live connection and the initial snapshot (recent campaigns).
///
/// # Example
///
/// ```ignore
/// let (mut conn, snapshot) = maillist::connect(
///     "https://lists.example.com",
///     "admin",
///     "password",
///     42,
/// ).await?;
///
/// println!("Connected to {}", snapshot.list_name);
/// for msg in &snapshot.messages {
///     println!("{}: {}", msg.subject, msg.sent_at);
/// }
/// ```
pub async fn connect(
    base_url: impl Into<String>,
    username: impl Into<String>,
    password: impl Into<String>,
    list_id: u32,
) -> Result<(MailConnection, MailSnapshot), MailError> {
    let base_url = base_url.into();
    let username = username.into();
    let password = password.into();

    let http = Client::builder()
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            // Basic auth encoded into a default header so every request carries it.
            let credentials = base64_encode(&format!("{username}:{password}"));
            headers.insert(
                reqwest::header::AUTHORIZATION,
                format!("Basic {credentials}").parse().expect("valid header value"),
            );
            headers
        })
        .build()?;

    let list_name =
        MailTransport::fetch_list_name(&http, &base_url, list_id).await?;

    let mut transport = MailTransport {
        http,
        base_url: base_url.clone(),
        list_id,
        list_name: list_name.clone(),
        messages: Vec::new(),
        seq: 0,
    };

    transport.messages = transport.fetch_messages().await?;
    let initial_snapshot = transport.current_snapshot();

    let manifest = Manifest {
        identity: Identity::local(format!("maillist:{base_url}/lists/{list_id}")),
        name: list_name,
        substrate: None,
        metadata: serde_json::json!({
            "type": "maillist",
            "list_id": list_id,
            "base_url": base_url,
        }),
    };

    let conn = MailConnection::established(transport, manifest);
    Ok((conn, initial_snapshot))
}

/// Encode a string as URL-safe base64 (standard alphabet, no padding — MIME
/// padding is fine here since this goes in an HTTP header value).
fn base64_encode(input: &str) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;

        out.push(ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((n >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(n & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encode_rfc4648_vectors() {
        assert_eq!(base64_encode(""), "");
        assert_eq!(base64_encode("f"), "Zg==");
        assert_eq!(base64_encode("fo"), "Zm8=");
        assert_eq!(base64_encode("foo"), "Zm9v");
        assert_eq!(base64_encode("foobar"), "Zm9vYmFy");
        assert_eq!(base64_encode("admin:password"), "YWRtaW46cGFzc3dvcmQ=");
    }
}
