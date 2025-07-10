use std::time::Duration;

use tokio::sync::broadcast;
use tokio::net::TcpListener;
use tokio::time::sleep;
use serde_json;
use reqwest_eventsource::{EventSource, Event};
use futures_util::StreamExt;

use hit_with_gpt::server::Change;
use hit_with_gpt::streaming::{self, Broadcaster};
use hit_with_gpt::sync::apply_change;
use hit_with_gpt::object::{Blob, Object, Hashable};
use hit_with_gpt::storage::{read_object, OBJECT_DIR};
use httpmock::{Method::GET, MockServer};
use serial_test::serial;

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

#[tokio::test]
#[serial]
async fn applies_change_from_server() {
    use std::fs;

    let server = MockServer::start();
    fs::remove_dir_all(".hit").ok();
    let path = "synced.txt";
    let blob = Blob { content: b"hello".to_vec() };
    let obj = Object::Blob(blob.clone());
    let bytes = bincode::serialize(&obj).unwrap();
    let hash = obj.hash();

    let mock = server.mock(|when, then| {
        when.method(GET).path(format!("/objects/{hash}"));
        then.status(200).body(bytes.clone());
    });

    let client = reqwest::Client::new();
    let change = Change { hash: hash.clone(), path: path.into(), timestamp: 1 };
    apply_change(&client, &server.url(""), &change).await.unwrap();

    mock.assert();
    assert!(fs::metadata(format!("{}/{}", OBJECT_DIR, hash)).unwrap().is_file());
    let obj2 = read_object(&hash).unwrap();
    assert_eq!(obj2, obj);
    let content = fs::read(path).unwrap();
    assert_eq!(content, blob.content);
}

#[tokio::test]
#[serial]
async fn error_when_object_unreachable() {
    let client = reqwest::Client::new();
    let change = Change { hash: "abcd".into(), path: "nope".into(), timestamp: 0 };
    let err = apply_change(&client, "http://127.0.0.1:59999", &change).await;
    assert!(err.is_err());
}
