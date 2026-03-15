use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::store::{DashItem, DashStore};

#[derive(Clone)]
struct ServerState {
    store: DashStore,
    auth_token: String,
    app: AppHandle,
}

/// Push 受信リクエストボディ
#[derive(Deserialize)]
pub struct PushRequest {
    pub source_id: String,
    pub source_name: String,
    pub title: String,
    pub body: Option<String>,
    pub url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Serialize)]
struct PushResponse {
    status: &'static str,
    id: String,
}

/// axum サーバーをバックグラウンドで起動する
pub fn start(app: AppHandle, store: DashStore, port: u16, auth_token: String) {
    let state = ServerState {
        store,
        auth_token,
        app,
    };

    let router = Router::new()
        .route("/health", get(health))
        .route("/items", get(get_items_handler))
        .route("/push", post(push_handler))
        .with_state(Arc::new(state));

    tauri::async_runtime::spawn(async move {
        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
            .await
            .expect("failed to bind push server");
        eprintln!("[server] listening on 127.0.0.1:{port}");
        axum::serve(listener, router)
            .await
            .expect("push server error");
    });
}

async fn health() -> &'static str {
    "ok"
}

/// GET /items — 現在のダッシュボードアイテムを JSON で返す
async fn get_items_handler(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Bearer トークン検証
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let expected = format!("Bearer {}", state.auth_token);
    if auth != expected {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let items = state.store.all_items().await;
    let count = items.len();
    Ok(Json(serde_json::json!({ "count": count, "items": items })))
}

async fn push_handler(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    Json(payload): Json<PushRequest>,
) -> Result<Json<PushResponse>, StatusCode> {
    // Bearer トークン検証
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let expected = format!("Bearer {}", state.auth_token);
    if auth != expected {
        return Err(StatusCode::UNAUTHORIZED);
    }

    if payload.title.is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let id = Uuid::new_v4().to_string();
    let item = DashItem {
        id: id.clone(),
        source_id: payload.source_id,
        source_name: payload.source_name,
        title: payload.title,
        body: payload.body,
        url: payload.url,
        image_data: None,
        published_at: Local::now(),
        tags: payload.tags,
    };

    state.store.push(item).await;
    let _ = state.app.emit("dashboard_updated", ());

    Ok(Json(PushResponse { status: "ok", id }))
}
