use axum::response::sse::{Event, KeepAlive, Sse};
use axum::{Router, extract::State, routing::get};
use std::convert::Infallible;
use std::time::Duration;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

use crate::server::ChangeEvent;

#[derive(Clone)]
pub struct Broadcaster {
    tx: Sender<ChangeEvent>,
}

impl Broadcaster {
    pub fn new(tx: Sender<ChangeEvent>) -> Self {
        Self { tx }
    }

    pub fn subscribe(&self) -> Receiver<ChangeEvent> {
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
        .route("/events", get(sse_handler))
        .with_state(broadcaster)
}
