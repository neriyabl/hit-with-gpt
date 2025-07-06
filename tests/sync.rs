use std::time::Duration;

use tokio::sync::broadcast;
use tokio::net::TcpListener;
use tokio::time::sleep;
use serde_json;
use reqwest_eventsource::{EventSource, Event};
use futures_util::StreamExt;

use hit_with_gpt::server::Change;
use hit_with_gpt::streaming::{self, Broadcaster};

#[tokio::test]
async fn parses_sse_event() {
    let (tx, _) = broadcast::channel(8);
    let router = streaming::router(Broadcaster::new(tx.clone()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    // send a change after clients can connect
    tokio::spawn(async move {
        sleep(Duration::from_millis(100)).await;
        tx.send(Change { hash: "abcd".into(), path: "foo.txt".into(), timestamp: 1 }).unwrap();
    });

    let url = format!("http://{}/events", addr);
    let client = reqwest::Client::new();
    let mut es = EventSource::new(client.get(&url)).unwrap();
    // first event should be Open
    matches!(es.next().await.unwrap().unwrap(), Event::Open);
    if let Event::Message(msg) = es.next().await.unwrap().unwrap() {
        let change: Change = serde_json::from_str(&msg.data).unwrap();
        assert_eq!(change.hash, "abcd");
        assert_eq!(change.path, "foo.txt");
    } else {
        panic!("expected message event");
    }
}
