//! Forum server implementation.

use crate::protocol::{
    ForumImportResult, ForumPassport, ForumProfile, Reply, Thread, ThreadDetail, ThreadList,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use interconnect_core::{Identity, Manifest};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

struct StoredThread {
    thread: Thread,
    body: String,
    replies: Vec<Reply>,
}

struct ServerState {
    name: String,
    #[allow(dead_code)] // Stored for future use
    port: u16,
    threads: Vec<StoredThread>,
    users: HashMap<Identity, ForumProfile>,
    next_thread_id: u64,
    next_reply_id: u64,
}

impl ServerState {
    fn new(name: String, port: u16) -> Self {
        Self {
            name,
            port,
            threads: Vec::new(),
            users: HashMap::new(),
            next_thread_id: 1,
            next_reply_id: 1,
        }
    }

    fn apply_import_policy(&self, passport: &ForumPassport) -> ForumImportResult {
        // Simple policy: accept reputation but cap it, require minimum rep to post
        let reputation = passport.reputation.clamp(-100, 100);
        let can_post = reputation >= 0 || passport.post_count > 10;

        ForumImportResult {
            reputation,
            can_post,
            restriction_reason: if can_post {
                None
            } else {
                Some("New users with negative reputation must wait for approval".to_string())
            },
        }
    }

    fn get_or_create_user(&mut self, identity: &Identity, name: &str) -> &mut ForumProfile {
        let now = now();
        self.users.entry(identity.clone()).or_insert_with(|| ForumProfile {
            identity: identity.clone(),
            display_name: name.to_string(),
            post_count: 0,
            reputation: 0,
            joined_at: now,
        })
    }
}

type AppState = Arc<RwLock<ServerState>>;

pub async fn run(port: u16, name: String) -> anyhow::Result<()> {
    let state = Arc::new(RwLock::new(ServerState::new(name, port)));

    let app = Router::new()
        .route("/manifest", get(get_manifest))
        .route("/threads", get(list_threads))
        .route("/threads", post(create_thread))
        .route("/threads/{id}", get(get_thread))
        .route("/threads/{id}/reply", post(reply_to_thread))
        .route("/profile/{identity}", get(get_profile))
        // Federation
        .route("/import", post(import_user))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

async fn get_manifest(State(state): State<AppState>) -> Json<Manifest> {
    let s = state.read().await;
    Json(Manifest {
        identity: Identity::local(&s.name),
        name: s.name.clone(),
        substrate: None,
        metadata: serde_json::json!({
            "type": "forum",
            "version": "0.1",
            "thread_count": s.threads.len()
        }),
    })
}

#[derive(Deserialize)]
struct Pagination {
    page: Option<u32>,
    per_page: Option<u32>,
}

async fn list_threads(
    State(state): State<AppState>,
    Query(params): Query<Pagination>,
) -> Json<ThreadList> {
    let s = state.read().await;
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20).min(100);

    let total = s.threads.len() as u32;
    let start = ((page - 1) * per_page) as usize;
    let threads: Vec<Thread> = s
        .threads
        .iter()
        .rev() // newest first
        .skip(start)
        .take(per_page as usize)
        .map(|t| t.thread.clone())
        .collect();

    Json(ThreadList {
        threads,
        total,
        page,
        per_page,
    })
}

async fn get_thread(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    Query(params): Query<Pagination>,
) -> Result<Json<ThreadDetail>, StatusCode> {
    let s = state.read().await;
    let stored = s
        .threads
        .iter()
        .find(|t| t.thread.id == id)
        .ok_or(StatusCode::NOT_FOUND)?;

    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(50).min(100);
    let start = ((page - 1) * per_page) as usize;

    let replies: Vec<Reply> = stored
        .replies
        .iter()
        .skip(start)
        .take(per_page as usize)
        .cloned()
        .collect();

    Ok(Json(ThreadDetail {
        thread: stored.thread.clone(),
        body: stored.body.clone(),
        replies,
        total_replies: stored.replies.len() as u32,
        page,
        per_page,
    }))
}

#[derive(Deserialize)]
struct CreateThreadRequest {
    title: String,
    body: String,
    #[serde(default)]
    author_name: String,
}

async fn create_thread(
    State(state): State<AppState>,
    Json(req): Json<CreateThreadRequest>,
) -> Result<Json<Thread>, StatusCode> {
    let mut s = state.write().await;

    // For demo, use local identity
    let identity = Identity::local(&req.author_name);
    let author_name = if req.author_name.is_empty() {
        "Anonymous".to_string()
    } else {
        req.author_name
    };

    let user = s.get_or_create_user(&identity, &author_name);
    user.post_count += 1;

    let now = now();
    let thread = Thread {
        id: s.next_thread_id,
        title: req.title,
        author: identity,
        author_name,
        created_at: now,
        reply_count: 0,
        last_activity: now,
    };
    s.next_thread_id += 1;

    let stored = StoredThread {
        thread: thread.clone(),
        body: req.body,
        replies: Vec::new(),
    };
    s.threads.push(stored);

    tracing::info!("New thread #{}: {}", thread.id, thread.title);
    Ok(Json(thread))
}

#[derive(Deserialize)]
struct ReplyRequest {
    body: String,
    #[serde(default)]
    author_name: String,
    parent_id: Option<u64>,
}

async fn reply_to_thread(
    State(state): State<AppState>,
    Path(thread_id): Path<u64>,
    Json(req): Json<ReplyRequest>,
) -> Result<Json<Reply>, StatusCode> {
    let mut s = state.write().await;

    // Check thread exists first
    if !s.threads.iter().any(|t| t.thread.id == thread_id) {
        return Err(StatusCode::NOT_FOUND);
    }

    let identity = Identity::local(&req.author_name);
    let author_name = if req.author_name.is_empty() {
        "Anonymous".to_string()
    } else {
        req.author_name
    };

    let now = now();
    let reply_id = s.next_reply_id;
    s.next_reply_id += 1;

    let reply = Reply {
        id: reply_id,
        thread_id,
        author: identity.clone(),
        author_name: author_name.clone(),
        body: req.body,
        created_at: now,
        parent_id: req.parent_id,
    };

    // Now we can safely borrow threads mutably
    if let Some(stored) = s.threads.iter_mut().find(|t| t.thread.id == thread_id) {
        stored.thread.reply_count += 1;
        stored.thread.last_activity = now;
        stored.replies.push(reply.clone());
    }

    let user = s.get_or_create_user(&identity, &author_name);
    user.post_count += 1;

    tracing::info!("New reply in thread #{}", thread_id);
    Ok(Json(reply))
}

async fn get_profile(
    State(state): State<AppState>,
    Path(identity_str): Path<String>,
) -> Result<Json<ForumProfile>, StatusCode> {
    let identity: Identity = identity_str.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    let s = state.read().await;
    let profile = s.users.get(&identity).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(profile.clone()))
}

async fn import_user(
    State(state): State<AppState>,
    Json(passport): Json<ForumPassport>,
) -> Json<ForumImportResult> {
    let s = state.read().await;
    let result = s.apply_import_policy(&passport);
    tracing::info!(
        "Import request from {}: reputation {} -> {}, can_post: {}",
        passport.display_name,
        passport.reputation,
        result.reputation,
        result.can_post
    );
    Json(result)
}
