use axum::{
    extract::{rejection::JsonRejection, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use tokio::sync::broadcast;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub hash: String,
    pub path: String,
    pub timestamp: u64,
}

#[derive(Clone, Default)]
pub struct ChangeStore {
    pub changes: Arc<Mutex<Vec<Change>>>,
}

#[derive(Clone)]
pub struct AppState {
    pub store: ChangeStore,
    pub broadcaster: broadcast::Sender<Change>,
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
    if let Ok(mut vec) = state.store.changes.lock() {
        vec.push(change.clone());
    }
    if let Err(e) = state.broadcaster.send(change.clone()) {
        tracing::warn!("failed to broadcast change: {}", e);
    }
    Ok(Json(json!({"accepted": true})))
}

fn app(state: AppState) -> Router {
    let changes = Router::new()
        .route("/changes", post(change_handler))
        .with_state(state.clone());
    let stream = crate::streaming::router(crate::streaming::Broadcaster::new(
        state.broadcaster.clone(),
    ));
    changes.merge(stream)
}

pub async fn start_server() {
    tracing_subscriber::fmt::init();
    let store = ChangeStore::default();
    let (tx, _) = broadcast::channel(100);
    let state = AppState { store, broadcaster: tx };
    let app = app(state);
    let addr = "0.0.0.0:8888";
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::info!("listening on http://{}", addr);
    axum::serve(listener, app)
        .await
        .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{Request, StatusCode};
    use axum::body::Body;
    use tokio::sync::broadcast;
    use tokio_stream::StreamExt;
    use tower::ServiceExt; // for `oneshot`
    use serde_json::{json, Value};

    #[tokio::test]
    async fn accepts_post() {
        let store = ChangeStore::default();
        let (tx, _) = broadcast::channel(8);
        let state = AppState { store: store.clone(), broadcaster: tx };
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
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v, json!({"accepted": true}));
        assert_eq!(store.changes.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn rejects_invalid_json() {
        let store = ChangeStore::default();
        let (tx, _) = broadcast::channel(8);
        let state = AppState { store: store.clone(), broadcaster: tx };
        let app = app(state);

        let req = Request::builder()
            .method("POST")
            .uri("/changes")
            .header("content-type", "application/json")
            .body(Body::from("{ not json"))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(store.changes.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn rejects_missing_field() {
        let store = ChangeStore::default();
        let (tx, _) = broadcast::channel(8);
        let state = AppState { store: store.clone(), broadcaster: tx };
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
        assert_eq!(store.changes.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn stores_multiple_changes() {
        let store = ChangeStore::default();
        let (tx, _) = broadcast::channel(8);
        let state = AppState { store: store.clone(), broadcaster: tx };
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
        assert_eq!(store.changes.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn streams_changes() {
        let store = ChangeStore::default();
        let (tx, _) = broadcast::channel(8);
        let state = AppState { store: store.clone(), broadcaster: tx.clone() };
        let app = app(state);

        let req = Request::builder()
            .uri("/stream")
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

        let change = Change { hash: "c1".into(), path: "f".into(), timestamp: 1 };
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
        let streamed: Change = serde_json::from_str(json_str).unwrap();
        assert_eq!(streamed, change);
    }
}
