use crate::commit::{Commit, CommitStore};
use crate::object::{Object, Hashable};
use crate::storage::{write_object, read_object};
use axum::{
    Json, Router,
    extract::{State, Path, rejection::JsonRejection},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    routing::post,
    routing::put,
    body::Bytes,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::broadcast;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub hash: String,
    pub path: String,
    pub timestamp: u64,
}

#[derive(Clone)]
pub struct AppState {
    pub commits: CommitStore,
    pub broadcaster: broadcast::Sender<ChangeEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ChangeEvent {
    pub change: Change,
    pub commit_id: u64,
}

async fn change_handler(
    State(state): State<AppState>,
    payload: Result<Json<Change>, JsonRejection>,
) -> Result<impl IntoResponse, StatusCode> {
    let Json(change) = match payload {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("invalid change payload: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };
    tracing::info!("change received: {:?}", change);
    
    let commit = match state.commits.add_commit(change.clone()) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("failed to create commit: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // Check if this is actually a new commit (not a duplicate)
    let is_new_commit = state.commits.all()
        .map(|commits| commits.len() as u64 == commit.id)
        .unwrap_or(true);
    
    if is_new_commit {
        tracing::info!(id = commit.id, "new commit created");
        if let Err(e) = state.broadcaster.send(ChangeEvent {
            change: change.clone(),
            commit_id: commit.id,
        }) {
            tracing::warn!("failed to broadcast change: {}", e);
        }
    } else {
        tracing::info!(id = commit.id, "duplicate change ignored");
    }
    
    Ok(Json(json!({"accepted": true})))
}

async fn commits_handler(State(state): State<AppState>) -> Result<Json<Vec<Commit>>, StatusCode> {
    match state.commits.all() {
        Ok(list) => Ok(Json(list)),
        Err(e) => {
            tracing::error!("failed to fetch commits: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn latest_commit_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.commits.latest() {
        Ok(Some(c)) => Json(c).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!("failed to fetch latest commit: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn store_object_handler(
    Path(hash): Path<String>,
    body: Bytes,
) -> Result<impl IntoResponse, StatusCode> {
    // Deserialize the object from the request body
    let obj: Object = match bincode::deserialize(&body) {
        Ok(obj) => obj,
        Err(e) => {
            tracing::warn!("failed to deserialize object: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };
    
    // Verify the hash matches
    if obj.hash() != hash {
        tracing::warn!("hash mismatch: expected {}, got {}", hash, obj.hash());
        return Err(StatusCode::BAD_REQUEST);
    }
    
    // Store the object
    if let Err(e) = write_object(&obj) {
        tracing::error!("failed to store object {}: {}", hash, e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    
    tracing::info!("stored object {}", hash);
    Ok(Json(json!({"stored": true})))
}

async fn get_object_handler(Path(hash): Path<String>) -> Result<impl IntoResponse, StatusCode> {
    match read_object(&hash) {
        Ok(obj) => {
            // Serialize the object for response
            match bincode::serialize(&obj) {
                Ok(bytes) => Ok((
                    [("Content-Type", "application/octet-stream")],
                    bytes,
                ).into_response()),
                Err(e) => {
                    tracing::error!("failed to serialize object {}: {}", hash, e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Err(e) => {
            tracing::warn!("object {} not found: {}", hash, e);
            Err(StatusCode::NOT_FOUND)
        }
    }
}

pub fn app(state: AppState) -> Router {
    let changes = Router::new()
        .route("/changes", post(change_handler))
        .route("/commits", get(commits_handler))
        .route("/commits/latest", get(latest_commit_handler))
        .route("/objects/:hash", put(store_object_handler))
        .route("/objects/:hash", get(get_object_handler))
        .with_state(state.clone());
    let stream = crate::streaming::router(crate::streaming::Broadcaster::new(
        state.broadcaster.clone(),
    ));
    changes.merge(stream)
}

use std::error::Error;

pub async fn start_server() -> Result<(), Box<dyn Error>> {
    std::fs::create_dir_all(".hit")?;
    std::fs::create_dir_all(".hit/objects")?;
    let commits = CommitStore::with_log(".hit/commits.log").map_err(|e| {
        tracing::error!("failed to initialize commit log: {}", e);
        e
    })?;
    let (tx, _) = broadcast::channel(100);
    let state = AppState {
        commits,
        broadcaster: tx,
    };
    let app = app(state);
    let addr = "0.0.0.0:8888";
    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
        tracing::error!("failed to bind to {}: {}", addr, e);
        e
    })?;
    tracing::info!("listening on http://{}", addr);
    axum::serve(listener, app).await.map_err(|e| {
        tracing::error!("server error: {}", e);
        e
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::{Value, json};
    use tokio::sync::broadcast;
    use tokio_stream::StreamExt;
    use tower::ServiceExt; // for `oneshot`

    #[tokio::test]
    async fn accepts_post() {
        let commits = CommitStore::default();
        let (tx, _) = broadcast::channel(8);
        let state = AppState {
            commits: commits.clone(),
            broadcaster: tx,
        };
        let app = app(state);

        let change = Change {
            hash: "abc".into(),
            path: "src/lib.rs".into(),
            timestamp: 1,
        };
        let req = Request::builder()
            .method("POST")
            .uri("/changes")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&change).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v, json!({"accepted": true}));
        assert_eq!(commits.all().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn rejects_invalid_json() {
        let commits = CommitStore::default();
        let (tx, _) = broadcast::channel(8);
        let state = AppState {
            commits: commits.clone(),
            broadcaster: tx,
        };
        let app = app(state);

        let req = Request::builder()
            .method("POST")
            .uri("/changes")
            .header("content-type", "application/json")
            .body(Body::from("{ not json"))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(commits.all().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn rejects_missing_field() {
        let commits = CommitStore::default();
        let (tx, _) = broadcast::channel(8);
        let state = AppState {
            commits: commits.clone(),
            broadcaster: tx,
        };
        let app = app(state);

        let body = json!({"path": "x", "timestamp": 1});
        let req = Request::builder()
            .method("POST")
            .uri("/changes")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(commits.all().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn returns_500_on_commit_error() {
        let commits = CommitStore::default();
        // poison the mutex
        {
            let c = commits.clone();
            std::thread::spawn(move || {
                let _guard = c.commits.lock().unwrap();
                panic!("boom");
            })
            .join()
            .ok();
        }
        let (tx, _) = broadcast::channel(8);
        let state = AppState {
            commits: commits.clone(),
            broadcaster: tx,
        };
        let app = app(state);

        let change = Change {
            hash: "x".into(),
            path: "f".into(),
            timestamp: 1,
        };
        let req = Request::builder()
            .method("POST")
            .uri("/changes")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&change).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn creates_commit_on_change() {
        let commits = CommitStore::default();
        let (tx, _) = broadcast::channel(8);
        let state = AppState {
            commits: commits.clone(),
            broadcaster: tx,
        };
        let app = app(state);

        let change = Change {
            hash: "h1".into(),
            path: "f".into(),
            timestamp: 1,
        };
        let req = Request::builder()
            .method("POST")
            .uri("/changes")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&change).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let commits_vec = commits.commits.lock().unwrap();
        assert_eq!(commits_vec.len(), 1);
        assert_eq!(commits_vec[0].id, 1);
    }

    #[tokio::test]
    async fn stores_multiple_changes() {
        let commits = CommitStore::default();
        let (tx, _) = broadcast::channel(8);
        let state = AppState {
            commits: commits.clone(),
            broadcaster: tx,
        };
        let app = app(state);

        for i in 0..2 {
            let change = Change {
                hash: format!("h{i}"),
                path: "file".into(),
                timestamp: i,
            };
            let req = Request::builder()
                .method("POST")
                .uri("/changes")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&change).unwrap()))
                .unwrap();
            let resp = app.clone().oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }
        assert_eq!(commits.all().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn streams_changes() {
        let commits = CommitStore::default();
        let (tx, _) = broadcast::channel(8);
        let state = AppState {
            commits: commits.clone(),
            broadcaster: tx.clone(),
        };
        let app = app(state);

        let req = Request::builder()
            .uri("/events")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let mut stream = resp.into_body().into_data_stream();
        let reader = tokio::spawn(async move {
            let mut bytes = Vec::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.unwrap();
                bytes.extend_from_slice(&chunk);
                if bytes.ends_with(b"\n\n") {
                    break;
                }
            }
            String::from_utf8(bytes).unwrap()
        });

        let change = Change {
            hash: "c1".into(),
            path: "f".into(),
            timestamp: 1,
        };
        let req = Request::builder()
            .method("POST")
            .uri("/changes")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&change).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let data = reader.await.unwrap();
        assert!(data.starts_with("data: "));
        let json_str = data.trim_start_matches("data: ").trim();
        let streamed: ChangeEvent = serde_json::from_str(json_str).unwrap();
        assert_eq!(streamed.change, change);
        assert_eq!(streamed.commit_id, 1);
    }

    #[tokio::test]
    async fn commit_history_endpoint() {
        let commits = CommitStore::default();
        let (tx, _) = broadcast::channel(8);
        let state = AppState {
            commits: commits.clone(),
            broadcaster: tx,
        };
        let app = app(state);

        let change = Change {
            hash: "c1".into(),
            path: "f".into(),
            timestamp: 1,
        };
        let req = Request::builder()
            .method("POST")
            .uri("/changes")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&change).unwrap()))
            .unwrap();
        app.clone().oneshot(req).await.unwrap();

        let req = Request::builder()
            .uri("/commits")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let list: Vec<Commit> = serde_json::from_slice(&body).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, 1);
    }

    #[tokio::test]
    async fn latest_commit_endpoint() {
        let commits = CommitStore::default();
        let (tx, _) = broadcast::channel(8);
        let state = AppState {
            commits: commits.clone(),
            broadcaster: tx,
        };
        let app = app(state);

        let change = Change {
            hash: "c1".into(),
            path: "f".into(),
            timestamp: 1,
        };
        let req = Request::builder()
            .method("POST")
            .uri("/changes")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&change).unwrap()))
            .unwrap();
        app.clone().oneshot(req).await.unwrap();

        let req = Request::builder()
            .uri("/commits/latest")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let commit: Commit = serde_json::from_slice(&body).unwrap();
        assert_eq!(commit.id, 1);
    }
}
