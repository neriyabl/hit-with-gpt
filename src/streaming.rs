use axum::{extract::State, routing::get, Router};
use axum::response::sse::{Event, KeepAlive, Sse};
use std::convert::Infallible;
use std::time::Duration;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

use crate::server::Change;

#[derive(Clone)]
pub struct Broadcaster {
    tx: Sender<Change>,
}

impl Broadcaster {
    pub fn new(tx: Sender<Change>) -> Self {
        Self { tx }
    }

    pub fn subscribe(&self) -> Receiver<Change> {
        self.tx.subscribe()
    }
}

pub async fn sse_handler(
    State(b): State<Broadcaster>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = b.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(change) => match serde_json::to_string(&change) {
            Ok(d) => Some(Ok(Event::default().data(d))),
            Err(e) => {
                tracing::warn!("failed to serialize change: {}", e);
                None
            }
        },
        Err(e) => {
            tracing::warn!("broadcast error: {}", e);
            None
        }
    });
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

pub fn router(broadcaster: Broadcaster) -> Router {
    Router::new()
        .route("/stream", get(sse_handler))
        .with_state(broadcaster)
}

