//! Listmonk polling transport.
//!
//! Presents a Listmonk mailing list as an Interconnect `Transport`. Polling
//! for new campaigns produces `ServerWire<MailSnapshot>` bytes;
//! `ClientWire<MailIntent>` bytes become Listmonk API calls.

use crate::types::{MailError, MailIntent, MailMessage, MailSnapshot};
use interconnect_core::{ClientWire, ServerWire, Transport};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tokio::time::sleep;

/// Maximum number of campaigns kept in the snapshot.
pub const MAX_MESSAGES: usize = 50;

/// Poll interval for new campaigns.
const POLL_INTERVAL: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// Listmonk API response shapes
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct CampaignsResponse {
    data: CampaignsData,
}

#[derive(Debug, Deserialize)]
struct CampaignsData {
    results: Vec<CampaignRecord>,
}

#[derive(Debug, Deserialize)]
struct CampaignRecord {
    id: u32,
    subject: String,
    body: String,
    /// ISO 8601 timestamp; may be null if not yet sent.
    send_at: Option<String>,
    /// Fallback creation timestamp (always present).
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct ListRecord {
    #[allow(dead_code)]
    id: u32,
    name: String,
}

#[derive(Debug, Deserialize)]
struct ListResponse {
    data: ListRecord,
}

#[derive(Debug, Deserialize)]
struct CreateCampaignResponse {
    data: CampaignRecord,
}

// ---------------------------------------------------------------------------
// Transport
// ---------------------------------------------------------------------------

pub struct MailTransport {
    pub(crate) http: Client,
    pub(crate) base_url: String,
    pub(crate) list_id: u32,
    pub(crate) list_name: String,
    pub(crate) messages: Vec<MailMessage>,
    pub(crate) seq: u64,
}

impl MailTransport {
    pub(crate) fn current_snapshot(&self) -> MailSnapshot {
        MailSnapshot {
            list_id: self.list_id,
            list_name: self.list_name.clone(),
            base_url: self.base_url.clone(),
            messages: self.messages.clone(),
        }
    }

    /// Fetch up to `MAX_MESSAGES` sent campaigns for the list, oldest first.
    pub(crate) async fn fetch_messages(&self) -> Result<Vec<MailMessage>, MailError> {
        let url = format!("{}/api/campaigns", self.base_url);
        let resp: CampaignsResponse = self
            .http
            .get(&url)
            .query(&[
                ("page", "1"),
                ("per_page", &MAX_MESSAGES.to_string()),
                ("status", "sent"),
                ("list_id", &self.list_id.to_string()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let mut messages: Vec<MailMessage> = resp
            .data
            .results
            .into_iter()
            .map(|r| {
                let sent_at = parse_timestamp(r.send_at.as_deref().unwrap_or(&r.created_at));
                MailMessage { id: r.id, subject: r.subject, body: r.body, sent_at }
            })
            .collect();

        // Oldest first.
        messages.sort_by_key(|m| m.sent_at);

        Ok(messages)
    }

    /// Fetch list metadata for the manifest name.
    pub(crate) async fn fetch_list_name(
        http: &Client,
        base_url: &str,
        list_id: u32,
    ) -> Result<String, MailError> {
        let url = format!("{base_url}/api/lists/{list_id}");
        let resp: ListResponse =
            http.get(&url).send().await?.error_for_status()?.json().await?;
        Ok(resp.data.name)
    }
}

impl Transport for MailTransport {
    type Error = MailError;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let wire: ClientWire<MailIntent> = serde_json::from_slice(data)?;
        // Auth, Ping, TransferRequest — not applicable for platform connectors.
        if let ClientWire::Intent(MailIntent::SendMessage { subject, body }) = wire {
            // Create the campaign.
            let create_url = format!("{}/api/campaigns", self.base_url);
            let payload = serde_json::json!({
                "name": &subject,
                "subject": &subject,
                "lists": [self.list_id],
                "type": "regular",
                "content_type": "plain_text",
                "body": &body,
            });
            let resp: CreateCampaignResponse = self
                .http
                .post(&create_url)
                .json(&payload)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;

            // Schedule the campaign to send now by updating its status.
            let campaign_id = resp.data.id;
            let status_url =
                format!("{}/api/campaigns/{}/status", self.base_url, campaign_id);
            self.http
                .put(&status_url)
                .json(&serde_json::json!({ "status": "running" }))
                .send()
                .await?
                .error_for_status()?;
        }
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<Vec<u8>>, Self::Error> {
        loop {
            sleep(POLL_INTERVAL).await;

            let latest = self.fetch_messages().await?;
            if latest.len() > self.messages.len() {
                self.messages = latest;
                let snapshot = self.current_snapshot();
                let wire = ServerWire::<MailSnapshot>::Snapshot {
                    seq: self.seq,
                    data: snapshot,
                };
                self.seq += 1;
                return Ok(Some(serde_json::to_vec(&wire)?));
            }
        }
    }
}

/// Parse an ISO 8601 timestamp string into a Unix timestamp (seconds).
///
/// Accepts the format Listmonk uses: `2024-01-15T10:30:00Z` or with offset.
/// Returns 0 on parse failure rather than propagating an error.
fn parse_timestamp(s: &str) -> u64 {
    // Try a simple hand-rolled parse for the common Listmonk format rather
    // than pulling in a full date-time crate.
    //
    // Listmonk emits RFC 3339 / ISO 8601 strings. We extract the components
    // directly and convert to a Unix timestamp.
    parse_rfc3339(s).unwrap_or(0)
}

/// Minimal RFC 3339 parser that returns seconds since the Unix epoch.
///
/// Handles `YYYY-MM-DDTHH:MM:SSZ` and `YYYY-MM-DDTHH:MM:SS+HH:MM` forms.
/// Ignores sub-second precision. Returns `None` on any parse failure.
fn parse_rfc3339(s: &str) -> Option<u64> {
    // Require at least "YYYY-MM-DDTHH:MM:SS" (19 chars).
    if s.len() < 19 {
        return None;
    }
    let year: i64 = s[0..4].parse().ok()?;
    let month: i64 = s[5..7].parse().ok()?;
    let day: i64 = s[8..10].parse().ok()?;
    let hour: i64 = s[11..13].parse().ok()?;
    let min: i64 = s[14..16].parse().ok()?;
    let sec: i64 = s[17..19].parse().ok()?;

    // Parse UTC offset (seconds) from the suffix, e.g. `Z`, `+05:30`, `-07:00`.
    let offset_secs: i64 = if s.len() > 19 {
        let suffix = &s[19..];
        // Skip any fractional seconds.
        let suffix = if let Some(rest) = suffix.strip_prefix('.') {
            // Skip digits.
            let digits_end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
            &suffix[1 + digits_end..]
        } else {
            suffix
        };

        if suffix == "Z" || suffix.is_empty() {
            0
        } else if suffix.starts_with('+') || suffix.starts_with('-') {
            let sign: i64 = if suffix.starts_with('-') { -1 } else { 1 };
            if suffix.len() >= 6 {
                let oh: i64 = suffix[1..3].parse().ok()?;
                let om: i64 = suffix[4..6].parse().ok()?;
                sign * (oh * 3600 + om * 60)
            } else {
                0
            }
        } else {
            0
        }
    } else {
        0
    };

    // Days from the Unix epoch (1970-01-01) using the proleptic Gregorian calendar.
    let days = days_from_epoch(year, month, day)?;
    let secs = days * 86400 + hour * 3600 + min * 60 + sec - offset_secs;
    if secs < 0 {
        return None;
    }
    Some(secs as u64)
}

/// Number of days from 1970-01-01 to `year-month-day`.
fn days_from_epoch(year: i64, month: i64, day: i64) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Days in each month for a non-leap year.
    const DAYS_IN_MONTH: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let mut days = 0i64;
    // Full years since 1970.
    for y in 1970..year {
        days += if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 { 366 } else { 365 };
    }
    // Full months in the current year.
    for m in 0..(month - 1) {
        let extra = if m == 1 && is_leap { 1 } else { 0 };
        days += DAYS_IN_MONTH[m as usize] + extra;
    }
    days += day - 1;
    Some(days)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_utc_z() {
        // 2024-01-15 10:30:00 UTC = 1705314600
        assert_eq!(parse_rfc3339("2024-01-15T10:30:00Z"), Some(1_705_314_600));
    }

    #[test]
    fn parse_positive_offset() {
        // 2024-01-15 10:30:00+05:30 => UTC 05:00:00 same day
        // 2024-01-15 00:00:00 UTC = 1705276800; +5h = 1705294800
        assert_eq!(parse_rfc3339("2024-01-15T10:30:00+05:30"), Some(1_705_294_800));
    }

    #[test]
    fn parse_fractional_seconds() {
        assert_eq!(parse_rfc3339("2024-01-15T10:30:00.123456Z"), Some(1_705_314_600));
    }

    #[test]
    fn parse_epoch() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
    }

    #[test]
    fn parse_invalid_returns_zero() {
        assert_eq!(parse_timestamp("not-a-date"), 0);
    }
}
