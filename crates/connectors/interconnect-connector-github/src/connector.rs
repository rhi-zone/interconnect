//! High-level connector entry point.

use interconnect_client::Connection;
use interconnect_core::{Identity, Manifest};
use reqwest::Client;

use crate::transport::GithubTransport;
use crate::types::{GithubComment, GithubError, GithubIntent, GithubSnapshot};

pub type GithubConnection = Connection<GithubTransport, GithubIntent, GithubSnapshot>;

/// Connect to a GitHub Issue as an Interconnect room.
///
/// Fetches the issue and all comments on connect, then polls every 30s for
/// new comments. Intents are translated to GitHub REST API calls.
///
/// # Example
///
/// ```ignore
/// use interconnect_connector_github as github;
///
/// let (mut conn, snapshot) = github::connect(token, "owner", "repo", 42).await?;
///
/// println!("Issue: {}", snapshot.title);
/// for comment in &snapshot.comments {
///     println!("{}: {}", comment.author, comment.body);
/// }
///
/// conn.send_intent(github::GithubIntent::AddComment {
///     body: "hello from another room".to_string(),
/// }).await?;
/// ```
pub async fn connect(
    token: impl Into<String>,
    owner: impl Into<String>,
    repo: impl Into<String>,
    issue_number: u64,
) -> Result<(GithubConnection, GithubSnapshot), GithubError> {
    let token = token.into();
    let owner = owner.into();
    let repo = repo.into();

    let http = Client::new();

    // Fetch issue metadata.
    let (title, state) = fetch_issue_info(&http, &token, &owner, &repo, issue_number).await?;

    // Fetch initial comments.
    let comments = fetch_initial_comments(&http, &token, &owner, &repo, issue_number).await?;

    // Seed the cursor from the most recent comment timestamp.
    let last_updated_at = comments
        .iter()
        .map(|c| c.timestamp)
        .max()
        .unwrap_or(0)
        .to_string();

    let transport = GithubTransport {
        http,
        token,
        owner: owner.clone(),
        repo: repo.clone(),
        issue_number,
        issue_title: title.clone(),
        issue_state: state.clone(),
        last_updated_at,
        comments: comments.clone(),
        seq: 0,
    };

    let manifest = Manifest {
        identity: Identity::local(format!("github:{owner}/{repo}#{issue_number}")),
        name: title.clone(),
        substrate: None,
        metadata: serde_json::json!({
            "type": "github_issue",
            "owner": owner,
            "repo": repo,
            "issue_number": issue_number,
        }),
    };

    let initial_snapshot = GithubSnapshot {
        owner,
        repo,
        issue_number,
        title,
        state,
        comments,
    };

    let conn = GithubConnection::established(transport, manifest);
    Ok((conn, initial_snapshot))
}

/// Fetch issue title and state from the GitHub API.
async fn fetch_issue_info(
    http: &Client,
    token: &str,
    owner: &str,
    repo: &str,
    issue_number: u64,
) -> Result<(String, String), GithubError> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/issues/{issue_number}");
    let resp: serde_json::Value = http
        .get(&url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "interconnect-connector-github")
        .send()
        .await?
        .error_for_status()
        .map_err(GithubError::Http)?
        .json()
        .await?;

    let title = resp["title"].as_str().unwrap_or("").to_string();
    let state = resp["state"].as_str().unwrap_or("open").to_string();
    Ok((title, state))
}

/// Fetch all comments for the issue from the GitHub API (oldest-first).
async fn fetch_initial_comments(
    http: &Client,
    token: &str,
    owner: &str,
    repo: &str,
    issue_number: u64,
) -> Result<Vec<GithubComment>, GithubError> {
    use crate::transport::GithubTransport;

    // Reuse the transport's fetch path by constructing a temporary transport.
    // We use the same helper via a thin wrapper to avoid duplicating the
    // HTTP+deserialization logic.
    let transport = GithubTransport {
        http: http.clone(),
        token: token.to_string(),
        owner: owner.to_string(),
        repo: repo.to_string(),
        issue_number,
        issue_title: String::new(),
        issue_state: String::new(),
        last_updated_at: String::new(),
        comments: Vec::new(),
        seq: 0,
    };
    transport.fetch_comments().await
}
