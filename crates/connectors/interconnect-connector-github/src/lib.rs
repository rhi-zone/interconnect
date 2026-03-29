//! GitHub Issues connector for the Interconnect protocol.
//!
//! Presents a GitHub Issue as an Interconnect room. Comments are the message
//! history, matching the utteranc.es model. Clients receive comment snapshots
//! and send intents that become GitHub REST API calls.
//!
//! Uses personal access token authentication and the GitHub REST API.
//!
//! # Usage
//!
//! ```ignore
//! use interconnect_connector_github as github;
//!
//! let (mut conn, snapshot) = github::connect(token, "owner", "repo", 42).await?;
//!
//! println!("Issue: {}", snapshot.title);
//! for comment in &snapshot.comments {
//!     println!("{}: {}", comment.author, comment.body);
//! }
//!
//! // Add a comment from another room:
//! conn.send_intent(github::GithubIntent::AddComment {
//!     body: "hello from another room".to_string(),
//! }).await?;
//! ```

mod connector;
mod transport;
mod types;

pub use connector::{GithubConnection, connect};
pub use types::{GithubComment, GithubError, GithubIntent, GithubSnapshot};
