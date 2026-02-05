//! Microblog server implementation using HTTP (axum).

use crate::protocol::{BlogIntent, Post, Profile, Timeline};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use interconnect_core::{Identity, Manifest};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

struct ServerState {
    identity: Identity,
    name: String,
    port: u16,
    posts: Vec<Post>,
    following: HashSet<Identity>,
    next_id: u64,
}

impl ServerState {
    fn new(name: String, port: u16) -> Self {
        let identity = Identity::url(format!("{}@localhost:{}", name, port));
        Self {
            identity,
            name,
            port,
            posts: Vec::new(),
            following: HashSet::new(),
            next_id: 1,
        }
    }
}

type AppState = Arc<RwLock<ServerState>>;

pub async fn run(port: u16, name: String) -> anyhow::Result<()> {
    let state = Arc::new(RwLock::new(ServerState::new(name, port)));

    let app = Router::new()
        // Interconnect protocol endpoints
        .route("/manifest", get(get_manifest))
        .route("/timeline", get(get_timeline))
        .route("/profile", get(get_profile))
        // Intent endpoints (actions)
        .route("/post", post(create_post))
        .route("/follow", post(follow_user))
        .route("/unfollow", post(unfollow_user))
        // Federation: fetch from other servers
        .route("/feed/{server}/{user}", get(fetch_remote_feed))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// GET /manifest - Interconnect manifest
async fn get_manifest(State(state): State<AppState>) -> Json<Manifest> {
    let s = state.read().await;
    Json(Manifest {
        identity: s.identity.clone(),
        name: format!("{}@localhost:{}", s.name, s.port),
        substrate: None,
        metadata: serde_json::json!({
            "type": "microblog",
            "version": "0.1"
        }),
    })
}

/// GET /timeline - this server's posts
async fn get_timeline(State(state): State<AppState>) -> Json<Timeline> {
    let s = state.read().await;
    Json(Timeline {
        posts: s.posts.iter().rev().take(20).cloned().collect(),
        server_name: format!("localhost:{}", s.port),
    })
}

/// GET /profile - this user's profile
async fn get_profile(State(state): State<AppState>) -> Json<Profile> {
    let s = state.read().await;
    Json(Profile {
        identity: s.identity.clone(),
        display_name: s.name.clone(),
        bio: String::new(),
        post_count: s.posts.len() as u64,
    })
}

/// POST /post - create a new post
async fn create_post(
    State(state): State<AppState>,
    Json(intent): Json<BlogIntent>,
) -> Result<Json<Post>, StatusCode> {
    let BlogIntent::Post { text } = intent else {
        return Err(StatusCode::BAD_REQUEST);
    };

    let mut s = state.write().await;
    let post = Post {
        id: s.next_id,
        author: s.identity.clone(),
        text,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    s.next_id += 1;
    s.posts.push(post.clone());

    tracing::info!("New post #{}: {}", post.id, post.text);
    Ok(Json(post))
}

/// POST /follow - follow a user
async fn follow_user(
    State(state): State<AppState>,
    Json(intent): Json<BlogIntent>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let BlogIntent::Follow { target } = intent else {
        return Err(StatusCode::BAD_REQUEST);
    };

    let mut s = state.write().await;
    s.following.insert(target.clone());

    tracing::info!("Now following {}", target);
    Ok(Json(serde_json::json!({ "following": target.to_string() })))
}

/// POST /unfollow - unfollow a user
async fn unfollow_user(
    State(state): State<AppState>,
    Json(intent): Json<BlogIntent>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let BlogIntent::Unfollow { target } = intent else {
        return Err(StatusCode::BAD_REQUEST);
    };

    let mut s = state.write().await;
    s.following.remove(&target);

    tracing::info!("Unfollowed {}", target);
    Ok(Json(serde_json::json!({ "unfollowed": target.to_string() })))
}

/// GET /feed/:server/:user - fetch posts from another server
///
/// This demonstrates the "visit, don't replicate" model.
/// We fetch from the authoritative server on demand.
async fn fetch_remote_feed(
    Path((server, user)): Path<(String, String)>,
) -> Result<Json<Timeline>, StatusCode> {
    let url = format!("http://{}/timeline", server);

    tracing::info!("Fetching feed from {} for @{}", server, user);

    // In a real implementation, we'd use reqwest or similar
    // For now, just indicate what we'd do
    Ok(Json(Timeline {
        posts: vec![],
        server_name: format!("{} (fetch from {})", user, url),
    }))
}
