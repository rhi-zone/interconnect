//! GitHub-specific protocol types.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A comment on a GitHub Issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubComment {
    /// GitHub comment ID.
    pub id: u64,
    /// Login of the comment author.
    pub author: String,
    /// Markdown body of the comment.
    pub body: String,
    /// Unix timestamp (seconds) from `created_at`.
    pub timestamp: u64,
    /// Reaction counts keyed by reaction content (+1, -1, laugh, etc.).
    pub reactions: HashMap<String, u32>,
}

/// Snapshot of a GitHub Issue room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubSnapshot {
    pub owner: String,
    pub repo: String,
    pub issue_number: u64,
    /// Issue title.
    pub title: String,
    /// Issue state: "open" or "closed".
    pub state: String,
    /// Comments, oldest first.
    pub comments: Vec<GithubComment>,
}

/// Intents a client can send to a GitHub Issue room.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GithubIntent {
    /// Post a new comment on the issue.
    AddComment { body: String },
    /// React to an existing comment.
    React {
        comment_id: u64,
        /// One of: +1, -1, laugh, confused, heart, hooray, rocket, eyes
        reaction: String,
    },
    /// Close the issue.
    CloseIssue,
}

/// Errors from GitHub connector operations.
#[derive(Debug, thiserror::Error)]
pub enum GithubError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("github api error: {0}")]
    Api(String),
}

impl From<GithubError> for interconnect_client::ClientError {
    fn from(e: GithubError) -> Self {
        interconnect_client::ClientError::Other(Box::new(e))
    }
}
