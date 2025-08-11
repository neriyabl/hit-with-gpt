use hit_with_gpt::commit::CommitStore;
use hit_with_gpt::object::{Blob, Hashable, Object};
use hit_with_gpt::server::AppState;
use hit_with_gpt::storage::{read_object, write_object};
use hit_with_gpt::watcher::{handle_event, send_change_to_server, send_object_to_server};

use notify::Event;
use notify::event::{CreateKind, EventKind};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::sleep;

#[tokio::test]
async fn test_send_object_to_server_success() {
    // Start a test server
    let commits = CommitStore::default();
    let (tx, _) = broadcast::channel(100);
    let state = AppState {
        commits,
        broadcaster: tx,
    };
    let app = hit_with_gpt::server::app(state);

    // Start the server on a test port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server_url = format!("http://{}", addr);

    // Set the environment variable for the watcher
    unsafe {
        env::set_var("HIT_SERVER_URL", &server_url);
    }

    // Start the server in the background
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give the server time to start
    sleep(Duration::from_millis(100)).await;

    // Create test objects with different content types
    let test_cases = vec![
        b"Simple text content".to_vec(),
        b"".to_vec(), // Empty file
        b"Binary data: \x00\x01\x02\xFF".to_vec(),
        "Unicode content: 🦀 Rust is awesome! 中文"
            .as_bytes()
            .to_vec(),
    ];

    for content in test_cases {
        let blob = Blob {
            content: content.clone(),
        };
        let obj = Object::Blob(blob.clone());
        let hash = obj.hash();

        // Test sending the object to the server
        let obj_clone = obj.clone();
        let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
            send_object_to_server(&obj_clone).map_err(|e| e.to_string())
        })
        .await
        .unwrap();

        assert!(
            result.is_ok(),
            "Failed to send object to server: {:?}",
            result
        );

        // Verify the object was stored correctly
        let client = reqwest::Client::new();
        let get_url = format!("{}/objects/{}", server_url, hash);
        let resp = client.get(&get_url).send().await.unwrap();

        assert_eq!(resp.status(), reqwest::StatusCode::OK);

        let body = resp.bytes().await.unwrap();
        let retrieved_obj: Object = bincode::deserialize(&body).unwrap();

        match retrieved_obj {
            Object::Blob(retrieved_blob) => {
                assert_eq!(retrieved_blob.content, content);
            }
            _ => panic!("Expected Blob object"),
        }
    }

    // Clean up
    server_handle.abort();
    unsafe { env::remove_var("HIT_SERVER_URL") }
}

#[tokio::test]
async fn test_send_change_to_server_success() {
    // Start a test server
    let commits = CommitStore::default();
    let (tx, _) = broadcast::channel(100);
    let state = AppState {
        commits,
        broadcaster: tx,
    };
    let app = hit_with_gpt::server::app(state);

    // Start the server on a test port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server_url = format!("http://{}", addr);

    // Set the environment variable for the watcher
    unsafe { env::set_var("HIT_SERVER_URL", &server_url) };

    // Start the server in the background
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give the server time to start
    sleep(Duration::from_millis(100)).await;

    // Test sending change notification
    let test_hash = "test_hash_12345";
    let test_path = Path::new("test_file.txt");

    let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
        send_change_to_server(test_hash, test_path).map_err(|e| e.to_string())
    })
    .await
    .unwrap();

    assert!(
        result.is_ok(),
        "Failed to send change to server: {:?}",
        result
    );

    // Verify the change was recorded by checking commits
    let client = reqwest::Client::new();
    let commits_url = format!("{}/commits", server_url);
    let resp = client.get(&commits_url).send().await.unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let commits: Vec<hit_with_gpt::commit::Commit> = resp.json().await.unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].changes[0].hash, test_hash);
    assert_eq!(commits[0].changes[0].path, test_path.to_string_lossy());

    // Clean up
    server_handle.abort();
    unsafe { env::remove_var("HIT_SERVER_URL") };
}

#[test]
fn test_handle_event_creates_and_stores_object() {
    // Create a temporary directory for testing
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Change to the temporary directory
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(temp_path).unwrap();

    // Create .hit directory structure
    fs::create_dir_all(".hit/objects").unwrap();

    // Create a test file
    let test_file_path = temp_path.join("test_file.txt");
    let test_content = b"Test file content for handle_event";
    fs::write(&test_file_path, test_content).unwrap();

    // Create a mock event with relative path (since we changed to temp_path)
    let event = Event {
        kind: EventKind::Create(CreateKind::File),
        paths: vec![PathBuf::from("test_file.txt")], // Use relative path
        attrs: Default::default(),
    };

    // Set a dummy server URL to avoid network calls in this test
    unsafe { env::set_var("HIT_SERVER_URL", "http://localhost:99999") };

    // Handle the event
    let result = handle_event(event);
    assert!(result.is_ok(), "handle_event failed: {:?}", result);

    // Verify that the object was created and stored
    let expected_blob = Blob {
        content: test_content.to_vec(),
    };
    let expected_obj = Object::Blob(expected_blob);
    let expected_hash = expected_obj.hash();

    // Check that the object file exists
    let object_path = Path::new(".hit/objects").join(&expected_hash);
    assert!(object_path.exists(), "Object file was not created");

    // Verify the stored object content
    let stored_obj = read_object(&expected_hash).unwrap();
    match stored_obj {
        Object::Blob(blob) => {
            assert_eq!(blob.content, test_content);
        }
        _ => panic!("Expected Blob object"),
    }

    // Clean up
    env::set_current_dir(original_dir).unwrap();
    unsafe { env::remove_var("HIT_SERVER_URL") };
}

#[test]
fn test_handle_event_ignores_hit_directory() {
    // Create a temporary directory for testing
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Change to the temporary directory
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(temp_path).unwrap();

    // Create .hit directory structure
    fs::create_dir_all(".hit/objects").unwrap();

    // Create a file inside .hit directory
    let hit_file_path = temp_path.join(".hit/some_file");
    fs::write(&hit_file_path, b"Should be ignored").unwrap();

    // Create a mock event for the .hit file
    let event = Event {
        kind: EventKind::Create(CreateKind::File),
        paths: vec![hit_file_path],
        attrs: Default::default(),
    };

    // Count objects before handling event
    let objects_before = fs::read_dir(".hit/objects").unwrap().count();

    // Handle the event
    let result = handle_event(event);
    assert!(result.is_ok(), "handle_event failed: {:?}", result);

    // Verify no new objects were created
    let objects_after = fs::read_dir(".hit/objects").unwrap().count();
    assert_eq!(
        objects_before, objects_after,
        "Objects were created for .hit directory files"
    );

    // Clean up
    env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_handle_event_ignores_temporary_files() {
    // Create a temporary directory for testing
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Change to the temporary directory
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(temp_path).unwrap();

    // Create .hit directory structure
    fs::create_dir_all(".hit/objects").unwrap();

    // Test files that should be ignored
    let ignored_files = vec![
        "file.txt~", // Backup file
        "file.swp",  // Vim swap file
        "temp.tmp",  // Temporary file
    ];

    for ignored_file in ignored_files {
        let file_path = temp_path.join(ignored_file);
        fs::write(&file_path, b"Should be ignored").unwrap();

        // Create a mock event
        let event = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![file_path],
            attrs: Default::default(),
        };

        // Count objects before handling event
        let objects_before = fs::read_dir(".hit/objects").unwrap().count();

        // Handle the event
        let result = handle_event(event);
        assert!(
            result.is_ok(),
            "handle_event failed for {}: {:?}",
            ignored_file,
            result
        );

        // Verify no new objects were created
        let objects_after = fs::read_dir(".hit/objects").unwrap().count();
        assert_eq!(
            objects_before, objects_after,
            "Objects were created for ignored file: {}",
            ignored_file
        );
    }

    // Clean up
    env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_handle_event_skips_existing_objects() {
    // Create a temporary directory for testing
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Change to the temporary directory
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(temp_path).unwrap();

    // Create .hit directory structure
    fs::create_dir_all(".hit/objects").unwrap();

    // Create a test file
    let test_file_path = temp_path.join("existing_file.txt");
    let test_content = b"Content for existing object test";
    fs::write(&test_file_path, test_content).unwrap();

    // Pre-create the object
    let blob = Blob {
        content: test_content.to_vec(),
    };
    let obj = Object::Blob(blob);
    write_object(&obj).unwrap();

    // Set a dummy server URL to avoid network calls
    unsafe { env::set_var("HIT_SERVER_URL", "http://localhost:99999") };

    // Create a mock event
    let event = Event {
        kind: EventKind::Create(CreateKind::File),
        paths: vec![test_file_path],
        attrs: Default::default(),
    };

    // Handle the event - should succeed but not create duplicate
    let result = handle_event(event);
    assert!(result.is_ok(), "handle_event failed: {:?}", result);

    // Verify the object still exists and is correct
    let hash = obj.hash();
    let stored_obj = read_object(&hash).unwrap();
    match stored_obj {
        Object::Blob(stored_blob) => {
            assert_eq!(stored_blob.content, test_content);
        }
        _ => panic!("Expected Blob object"),
    }

    // Clean up
    env::set_current_dir(original_dir).unwrap();
    unsafe { env::remove_var("HIT_SERVER_URL") };
}

#[test]
fn test_handle_event_ignores_directories() {
    // Create a temporary directory for testing
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Change to the temporary directory
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(temp_path).unwrap();

    // Create .hit directory structure
    fs::create_dir_all(".hit/objects").unwrap();

    // Create a subdirectory
    let sub_dir_path = temp_path.join("subdirectory");
    fs::create_dir(&sub_dir_path).unwrap();

    // Create a mock event for directory creation
    let event = Event {
        kind: EventKind::Create(CreateKind::Folder),
        paths: vec![sub_dir_path],
        attrs: Default::default(),
    };

    // Count objects before handling event
    let objects_before = fs::read_dir(".hit/objects").unwrap().count();

    // Handle the event
    let result = handle_event(event);
    assert!(result.is_ok(), "handle_event failed: {:?}", result);

    // Verify no objects were created for directory
    let objects_after = fs::read_dir(".hit/objects").unwrap().count();
    assert_eq!(
        objects_before, objects_after,
        "Objects were created for directory"
    );

    // Clean up
    env::set_current_dir(original_dir).unwrap();
}
