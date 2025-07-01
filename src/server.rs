use axum::{
    extract::{rejection::JsonRejection, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Change {
    pub hash: String,
    pub path: String,
    pub timestamp: u64,
}

#[derive(Clone, Default)]
pub struct ChangeStore {
    pub changes: Arc<Mutex<Vec<Change>>>,
}

async fn change_handler(
    State(store): State<ChangeStore>,
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
    if let Ok(mut vec) = store.changes.lock() {
        vec.push(change);
    }
    Ok(Json(json!({"accepted": true})))
}

fn app(store: ChangeStore) -> Router {
    Router::new()
        .route("/changes", post(change_handler))
        .with_state(store)
}

pub async fn start_server() {
    tracing_subscriber::fmt::init();
    let store = ChangeStore::default();
    let app = app(store);
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
    use tower::ServiceExt; // for `oneshot`
    use serde_json::{json, Value};

    #[tokio::test]
    async fn accepts_post() {
        let store = ChangeStore::default();
        let app = app(store.clone());

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
        let app = app(store.clone());

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
        let app = app(store.clone());

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
        let app = app(store.clone());

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
}
