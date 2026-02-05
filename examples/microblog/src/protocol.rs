//! Microblog protocol types.

use interconnect_core::Identity;
use serde::{Deserialize, Serialize};

/// A post on the microblog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: u64,
    pub author: Identity,
    pub text: String,
    pub timestamp: u64,
}

/// Timeline snapshot - recent posts from this server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timeline {
    pub posts: Vec<Post>,
    pub server_name: String,
}

/// Intent for posting.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlogIntent {
    /// Create a new post.
    Post { text: String },
    /// Follow a user (on any server).
    Follow { target: Identity },
    /// Unfollow a user.
    Unfollow { target: Identity },
}

/// Profile that can be fetched across servers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub identity: Identity,
    pub display_name: String,
    pub bio: String,
    pub post_count: u64,
}

/// Passport for profile transfer (moving to a new server).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)] // Part of the protocol, not used in this demo
pub struct BlogPassport {
    pub identity: Identity,
    pub display_name: String,
    pub bio: String,
    /// List of followers to notify about the move.
    pub followers: Vec<Identity>,
}
