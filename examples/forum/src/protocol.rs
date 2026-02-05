//! Forum protocol types.

use interconnect_core::Identity;
use serde::{Deserialize, Serialize};

/// A forum thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub id: u64,
    pub title: String,
    pub author: Identity,
    pub author_name: String,
    pub created_at: u64,
    pub reply_count: u32,
    pub last_activity: u64,
}

/// A reply in a thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reply {
    pub id: u64,
    pub thread_id: u64,
    pub author: Identity,
    pub author_name: String,
    pub body: String,
    pub created_at: u64,
    /// Reply to another reply (for nested threads).
    pub parent_id: Option<u64>,
}

/// Thread with its replies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadDetail {
    pub thread: Thread,
    pub body: String,
    pub replies: Vec<Reply>,
    /// For pagination.
    pub total_replies: u32,
    pub page: u32,
    pub per_page: u32,
}

/// Paginated thread list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadList {
    pub threads: Vec<Thread>,
    pub total: u32,
    pub page: u32,
    pub per_page: u32,
}

/// Intent for forum actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)] // Part of the protocol, not used in this demo
pub enum ForumIntent {
    /// Create a new thread.
    CreateThread { title: String, body: String },
    /// Reply to a thread.
    Reply {
        thread_id: u64,
        body: String,
        parent_id: Option<u64>,
    },
    /// Edit a post (only your own).
    Edit { post_id: u64, body: String },
    /// Delete a post (only your own, or mod).
    Delete { post_id: u64 },
}

/// User profile for forums.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForumProfile {
    pub identity: Identity,
    pub display_name: String,
    pub post_count: u32,
    pub reputation: i32,
    pub joined_at: u64,
}

/// Passport for cross-forum posting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForumPassport {
    pub identity: Identity,
    pub display_name: String,
    pub home_forum: String,
    pub reputation: i32,
    pub post_count: u32,
}

#[allow(dead_code)] // Part of the protocol, not used in this demo
impl ForumPassport {
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        serde_json::from_slice(data).ok()
    }
}

/// Import policy result for forum reputation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForumImportResult {
    /// Accepted reputation (may be clamped or ignored).
    pub reputation: i32,
    /// Whether the user can post immediately or needs approval.
    pub can_post: bool,
    /// Reason if posting is restricted.
    pub restriction_reason: Option<String>,
}
