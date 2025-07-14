use std::time::Duration;

use futures_util::StreamExt;
use reqwest_eventsource::{Event, EventSource};
use serde_json;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::time::sleep;

use hit_with_gpt::object::{Blob, Hashable, Object};
use hit_with_gpt::server::Change;
use hit_with_gpt::storage::{OBJECT_DIR, read_object};
use hit_with_gpt::streaming::{self, Broadcaster};
use hit_with_gpt::sync::apply_change;
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
        tx.send(Change {
            hash: "abcd".into(),
            path: "foo.txt".into(),
            timestamp: 1,
        })
        .unwrap();
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
    let blob = Blob {
        content: b"hello".to_vec(),
    };
    let obj = Object::Blob(blob.clone());
    let bytes = bincode::serialize(&obj).unwrap();
    let hash = obj.hash();

    let mock = server.mock(|when, then| {
        when.method(GET).path(format!("/objects/{hash}"));
        then.status(200).body(bytes.clone());
    });

    let client = reqwest::Client::new();
    let change = Change {
        hash: hash.clone(),
        path: path.into(),
        timestamp: 1,
    };
    apply_change(&client, &server.url(""), &change)
        .await
        .unwrap();

    mock.assert();
    assert!(
        fs::metadata(format!("{}/{}", OBJECT_DIR, hash))
            .unwrap()
            .is_file()
    );
    let obj2 = read_object(&hash).unwrap();
    assert_eq!(obj2, obj);
    let content = fs::read(path).unwrap();
    assert_eq!(content, blob.content);
}

#[tokio::test]
#[serial]
async fn error_when_object_unreachable() {
    let client = reqwest::Client::new();
    let change = Change {
        hash: "abcd".into(),
        path: "nope".into(),
        timestamp: 0,
    };
    let err = apply_change(&client, "http://127.0.0.1:59999", &change).await;
    assert!(err.is_err());
}

#[tokio::test]
#[serial]
async fn rejects_path_traversal() {
    use std::fs;
    use std::path::Path;

    let server = MockServer::start();
    fs::remove_dir_all(".hit").ok();
    let blob = Blob {
        content: b"hi".to_vec(),
    };
    let obj = Object::Blob(blob.clone());
    let bytes = bincode::serialize(&obj).unwrap();
    let hash = obj.hash();

    server.mock(|when, then| {
        when.method(GET).path(format!("/objects/{hash}"));
        then.status(200).body(bytes.clone());
    });

    let client = reqwest::Client::new();
    let change = Change {
        hash: hash.clone(),
        path: "../evil.txt".into(),
        timestamp: 1,
    };
    let res = apply_change(&client, &server.url(""), &change).await;
    assert!(res.is_err());
    assert!(!Path::new("../evil.txt").exists());
}

#[tokio::test]
#[serial]
async fn rejects_non_blob_objects() {
    use std::fs;
    use std::path::Path;

    let server = MockServer::start();
    fs::remove_dir_all(".hit").ok();
    let obj = Object::Commit(hit_with_gpt::object::Commit {
        tree: hit_with_gpt::object::Tree { entries: vec![] },
        message: "msg".into(),
    });
    let bytes = bincode::serialize(&obj).unwrap();
    let hash = obj.hash();

    server.mock(|when, then| {
        when.method(GET).path(format!("/objects/{hash}"));
        then.status(200).body(bytes.clone());
    });

    let client = reqwest::Client::new();
    let change = Change {
        hash: hash.clone(),
        path: "commit.txt".into(),
        timestamp: 1,
    };
    let res = apply_change(&client, &server.url(""), &change).await;
    assert!(res.is_err());
    assert!(!Path::new("commit.txt").exists());
}

#[tokio::test]
#[serial]
async fn backs_up_existing_file() {
    use std::fs;

    let server = MockServer::start();
    fs::remove_dir_all(".hit").ok();
    fs::write("backup.txt", b"old").unwrap();
    let blob = Blob {
        content: b"new".to_vec(),
    };
    let obj = Object::Blob(blob.clone());
    let bytes = bincode::serialize(&obj).unwrap();
    let hash = obj.hash();

    server.mock(|when, then| {
        when.method(GET).path(format!("/objects/{hash}"));
        then.status(200).body(bytes.clone());
    });

    let client = reqwest::Client::new();
    let change = Change {
        hash,
        path: "backup.txt".into(),
        timestamp: 1,
    };
    apply_change(&client, &server.url(""), &change)
        .await
        .unwrap();

    assert_eq!(fs::read("backup.txt").unwrap(), blob.content);
    assert_eq!(fs::read("backup.bak").unwrap(), b"old");
}
