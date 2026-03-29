//! GitHub REST API transport.
//!
//! Presents a GitHub Issue as an Interconnect `Transport`. Polling
//! `GET /repos/{owner}/{repo}/issues/{issue_number}/comments` every 30s
//! produces `ServerWire<GithubSnapshot>` bytes; `ClientWire<GithubIntent>`
//! bytes become GitHub REST API calls.

use std::collections::HashMap;
use std::time::Duration;

use interconnect_core::{ClientWire, ServerWire, Transport};
use reqwest::Client;
use serde::Deserialize;
use tokio::time::sleep;

use crate::types::{GithubComment, GithubError, GithubIntent, GithubSnapshot};

/// Interval between comment polls.
const POLL_INTERVAL: Duration = Duration::from_secs(30);

pub struct GithubTransport {
    pub(crate) http: Client,
    pub(crate) token: String,
    pub(crate) owner: String,
    pub(crate) repo: String,
    pub(crate) issue_number: u64,
    pub(crate) issue_title: String,
    pub(crate) issue_state: String,
    /// The latest `updated_at` timestamp seen across all comments.
    pub(crate) last_updated_at: String,
    pub(crate) comments: Vec<GithubComment>,
    pub(crate) seq: u64,
}

/// Partial GitHub comment as returned by the REST API.
#[derive(Debug, Deserialize)]
struct ApiComment {
    id: u64,
    user: ApiUser,
    body: String,
    created_at: String,
    reactions: Option<ApiReactions>,
}

#[derive(Debug, Deserialize)]
struct ApiUser {
    login: String,
}

/// Reaction summary block returned inline with comments when
/// `Accept: application/vnd.github+json` is set.
#[derive(Debug, Deserialize)]
struct ApiReactions {
    #[serde(rename = "+1")]
    plus_one: Option<u32>,
    #[serde(rename = "-1")]
    minus_one: Option<u32>,
    laugh: Option<u32>,
    confused: Option<u32>,
    heart: Option<u32>,
    hooray: Option<u32>,
    rocket: Option<u32>,
    eyes: Option<u32>,
}

impl ApiReactions {
    fn into_map(self) -> HashMap<String, u32> {
        let mut map = HashMap::new();
        let fields = [
            ("+1", self.plus_one),
            ("-1", self.minus_one),
            ("laugh", self.laugh),
            ("confused", self.confused),
            ("heart", self.heart),
            ("hooray", self.hooray),
            ("rocket", self.rocket),
            ("eyes", self.eyes),
        ];
        for (key, val) in fields {
            if let Some(count) = val {
                if count > 0 {
                    map.insert(key.to_string(), count);
                }
            }
        }
        map
    }
}

/// Parse an ISO 8601 timestamp ("2024-01-15T12:34:56Z") into Unix seconds.
/// Returns 0 on parse failure.
fn parse_iso8601(s: &str) -> u64 {
    // Format: YYYY-MM-DDTHH:MM:SSZ (always UTC from GitHub)
    // Manual parse to avoid pulling in chrono or time.
    let s = s.trim_end_matches('Z');
    let parts: Vec<&str> = s.splitn(2, 'T').collect();
    if parts.len() != 2 {
        return 0;
    }
    let date_parts: Vec<u64> = parts[0].split('-').filter_map(|p| p.parse().ok()).collect();
    let time_parts: Vec<u64> = parts[1].split(':').filter_map(|p| p.parse().ok()).collect();
    if date_parts.len() != 3 || time_parts.len() != 3 {
        return 0;
    }
    let (year, month, day) = (date_parts[0], date_parts[1], date_parts[2]);
    let (hour, min, sec) = (time_parts[0], time_parts[1], time_parts[2]);

    // Days since Unix epoch via a simple algorithm (valid for modern dates).
    // Uses the algorithm from https://howardhinnant.github.io/date_algorithms.html
    let y = if month <= 2 { year - 1 } else { year };
    let m = month;
    let era: u64 = y / 400;
    let yoe: u64 = y - era * 400;
    let doy: u64 = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe: u64 = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days_since_epoch: i64 = (era as i64) * 146097 + (doe as i64) - 719468;

    if days_since_epoch < 0 {
        return 0;
    }
    (days_since_epoch as u64) * 86400 + hour * 3600 + min * 60 + sec
}

fn convert_comment(api: ApiComment) -> GithubComment {
    let reactions = api
        .reactions
        .map(|r| r.into_map())
        .unwrap_or_default();
    GithubComment {
        id: api.id,
        author: api.user.login,
        body: api.body,
        timestamp: parse_iso8601(&api.created_at),
        reactions,
    }
}

impl GithubTransport {
    pub(crate) fn current_snapshot(&self) -> GithubSnapshot {
        GithubSnapshot {
            owner: self.owner.clone(),
            repo: self.repo.clone(),
            issue_number: self.issue_number,
            title: self.issue_title.clone(),
            state: self.issue_state.clone(),
            comments: self.comments.clone(),
        }
    }

    /// Fetch all comments from the GitHub API and return them oldest-first.
    pub(crate) async fn fetch_comments(&self) -> Result<Vec<GithubComment>, GithubError> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/issues/{}/comments",
            self.owner, self.repo, self.issue_number
        );
        let api_comments: Vec<ApiComment> = self
            .http
            .get(&url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            // GitHub requires a User-Agent header.
            .header("User-Agent", "interconnect-connector-github")
            .query(&[("per_page", "100")])
            .send()
            .await?
            .error_for_status()
            .map_err(GithubError::Http)?
            .json()
            .await?;

        Ok(api_comments.into_iter().map(convert_comment).collect())
    }

}

impl Transport for GithubTransport {
    type Error = GithubError;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let wire: ClientWire<GithubIntent> = serde_json::from_slice(data)?;
        match wire {
            ClientWire::Intent(intent) => match intent {
                GithubIntent::AddComment { body } => {
                    let url = format!(
                        "https://api.github.com/repos/{}/{}/issues/{}/comments",
                        self.owner, self.repo, self.issue_number
                    );
                    self.http
                        .post(&url)
                        .bearer_auth(&self.token)
                        .header("Accept", "application/vnd.github+json")
                        .header("X-GitHub-Api-Version", "2022-11-28")
                        .header("User-Agent", "interconnect-connector-github")
                        .json(&serde_json::json!({ "body": body }))
                        .send()
                        .await?
                        .error_for_status()
                        .map_err(GithubError::Http)?;
                }
                GithubIntent::React { comment_id, reaction } => {
                    let url = format!(
                        "https://api.github.com/repos/{}/{}/issues/comments/{}/reactions",
                        self.owner, self.repo, comment_id
                    );
                    self.http
                        .post(&url)
                        .bearer_auth(&self.token)
                        .header("Accept", "application/vnd.github+json")
                        .header("X-GitHub-Api-Version", "2022-11-28")
                        .header("User-Agent", "interconnect-connector-github")
                        .json(&serde_json::json!({ "content": reaction }))
                        .send()
                        .await?
                        .error_for_status()
                        .map_err(GithubError::Http)?;
                }
                GithubIntent::CloseIssue => {
                    let url = format!(
                        "https://api.github.com/repos/{}/{}/issues/{}",
                        self.owner, self.repo, self.issue_number
                    );
                    let resp: serde_json::Value = self
                        .http
                        .patch(&url)
                        .bearer_auth(&self.token)
                        .header("Accept", "application/vnd.github+json")
                        .header("X-GitHub-Api-Version", "2022-11-28")
                        .header("User-Agent", "interconnect-connector-github")
                        .json(&serde_json::json!({ "state": "closed" }))
                        .send()
                        .await?
                        .error_for_status()
                        .map_err(GithubError::Http)?
                        .json()
                        .await?;
                    if let Some(state) = resp["state"].as_str() {
                        self.issue_state = state.to_string();
                    }
                }
            },
            // Auth, Ping, TransferRequest — not applicable for platform connectors.
            _ => {}
        }
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<Vec<u8>>, Self::Error> {
        loop {
            sleep(POLL_INTERVAL).await;

            let new_comments = self.fetch_comments().await?;

            // Determine the latest updated_at across the new set.
            let new_cursor = new_comments
                .iter()
                .map(|c| c.timestamp)
                .max()
                .unwrap_or(0)
                .to_string();

            // Emit a snapshot only when the comment list has changed.
            let changed = new_comments.len() != self.comments.len()
                || new_cursor != self.last_updated_at;

            if changed {
                self.comments = new_comments;
                self.last_updated_at = new_cursor;

                let snapshot = self.current_snapshot();
                let wire = ServerWire::<GithubSnapshot>::Snapshot {
                    seq: self.seq,
                    data: snapshot,
                };
                self.seq += 1;
                return Ok(Some(serde_json::to_vec(&wire)?));
            }
        }
    }
}
