use hit_with_gpt::object::{Blob, Object, Hashable};
use hit_with_gpt::watcher::send_object_to_server;
use hit_with_gpt::server::AppState;
use hit_with_gpt::commit::CommitStore;
use hit_with_gpt::storage::write_object;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use serde_json::json;
use std::env;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::sleep;
use tower::ServiceExt;

// Helper function to create a test app
fn create_test_app() -> Router {
    let commits = CommitStore::default();
    let (tx, _) = broadcast::channel(100);
    let state = AppState {
        commits,
        broadcaster: tx,
    };
    hit_with_gpt::server::app(state)
}

#[tokio::test]
async fn test_store_object_endpoint() {
    let app = create_test_app();
    
    // Create a test object
    let blob = Blob {
        content: b"Hello, world!".to_vec(),
    };
    let obj = Object::Blob(blob);
    let hash = obj.hash();
    
    // Serialize the object
    let serialized = bincode::serialize(&obj).unwrap();
    
    // Send PUT request to store the object
    let req = Request::builder()
        .method("PUT")
        .uri(format!("/objects/{}", hash))
        .header("Content-Type", "application/octet-stream")
        .body(Body::from(serialized))
        .unwrap();
    
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    
    // Verify the response body
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(response, json!({"stored": true}));
}

#[tokio::test]
async fn test_get_object_endpoint() {
    let app = create_test_app();
    
    // Create and store a test object locally first
    let blob = Blob {
        content: b"Test content for retrieval".to_vec(),
    };
    let obj = Object::Blob(blob.clone());
    let hash = obj.hash();
    
    // Store the object locally (simulating it was stored via PUT)
    write_object(&obj).unwrap();
    
    // Send GET request to retrieve the object
    let req = Request::builder()
        .method("GET")
        .uri(format!("/objects/{}", hash))
        .body(Body::empty())
        .unwrap();
    
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    
    // Verify the response content
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let retrieved_obj: Object = bincode::deserialize(&body).unwrap();
    
    match retrieved_obj {
        Object::Blob(retrieved_blob) => {
            assert_eq!(retrieved_blob.content, blob.content);
        }
        _ => panic!("Expected Blob object"),
    }
}

#[tokio::test]
async fn test_get_nonexistent_object() {
    let app = create_test_app();
    
    // Try to get an object that doesn't exist
    let fake_hash = "nonexistent_hash_12345";
    let req = Request::builder()
        .method("GET")
        .uri(format!("/objects/{}", fake_hash))
        .body(Body::empty())
        .unwrap();
    
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_store_object_with_hash_mismatch() {
    let app = create_test_app();
    
    // Create a test object
    let blob = Blob {
        content: b"Hello, world!".to_vec(),
    };
    let obj = Object::Blob(blob);
    let _correct_hash = obj.hash();
    let wrong_hash = "wrong_hash_12345";
    
    // Serialize the object
    let serialized = bincode::serialize(&obj).unwrap();
    
    // Send PUT request with wrong hash
    let req = Request::builder()
        .method("PUT")
        .uri(format!("/objects/{}", wrong_hash))
        .header("Content-Type", "application/octet-stream")
        .body(Body::from(serialized))
        .unwrap();
    
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_store_object_with_invalid_data() {
    let app = create_test_app();
    
    let hash = "some_hash";
    let invalid_data = b"not a valid serialized object";
    
    // Send PUT request with invalid data
    let req = Request::builder()
        .method("PUT")
        .uri(format!("/objects/{}", hash))
        .header("Content-Type", "application/octet-stream")
        .body(Body::from(invalid_data.as_slice()))
        .unwrap();
    
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_roundtrip_object_storage() {
    let app = create_test_app();
    
    // Create multiple test objects
    let test_cases = vec![
        b"Small content".to_vec(),
        b"".to_vec(), // Empty content
        b"Large content with special characters: \n\t\r\0\x01\xFF".to_vec(),
        (0..1000).map(|i| (i % 256) as u8).collect(), // Binary data
    ];
    
    for content in test_cases {
        let blob = Blob { content: content.clone() };
        let obj = Object::Blob(blob);
        let hash = obj.hash();
        
        // Serialize and store the object
        let serialized = bincode::serialize(&obj).unwrap();
        let store_req = Request::builder()
            .method("PUT")
            .uri(format!("/objects/{}", hash))
            .header("Content-Type", "application/octet-stream")
            .body(Body::from(serialized))
            .unwrap();
        
        let store_resp = app.clone().oneshot(store_req).await.unwrap();
        assert_eq!(store_resp.status(), StatusCode::OK);
        
        // Retrieve the object
        let get_req = Request::builder()
            .method("GET")
            .uri(format!("/objects/{}", hash))
            .body(Body::empty())
            .unwrap();
        
        let get_resp = app.clone().oneshot(get_req).await.unwrap();
        assert_eq!(get_resp.status(), StatusCode::OK);
        
        // Verify the content matches
        let body = axum::body::to_bytes(get_resp.into_body(), usize::MAX).await.unwrap();
        let retrieved_obj: Object = bincode::deserialize(&body).unwrap();
        
        match retrieved_obj {
            Object::Blob(retrieved_blob) => {
                assert_eq!(retrieved_blob.content, content);
            }
            _ => panic!("Expected Blob object"),
        }
    }
}

// Integration test for the watcher's send_object_to_server function
#[tokio::test]
async fn test_send_object_to_server_integration() {
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
    unsafe { env::set_var("HIT_SERVER_URL", &server_url); }
    
    // Start the server in the background
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    
    // Give the server time to start
    sleep(Duration::from_millis(100)).await;
    
    // Create a test object
    let blob = Blob {
        content: b"Integration test content".to_vec(),
    };
    let obj = Object::Blob(blob.clone());
    
    // Test sending the object to the server (run in blocking context)
    let obj_clone = obj.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
        send_object_to_server(&obj_clone).map_err(|e| e.to_string())
    }).await.unwrap();
    
    assert!(result.is_ok(), "Failed to send object to server: {:?}", result);
    
    // Verify the object was stored by trying to read it back
    let hash = obj.hash();
    let client = reqwest::Client::new();
    let get_url = format!("{}/objects/{}", server_url, hash);
    let resp = client.get(&get_url).send().await.unwrap();
    
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    
    let body = resp.bytes().await.unwrap();
    let retrieved_obj: Object = bincode::deserialize(&body).unwrap();
    
    match retrieved_obj {
        Object::Blob(retrieved_blob) => {
            assert_eq!(retrieved_blob.content, blob.content);
        }
        _ => panic!("Expected Blob object"),
    }
    
    // Clean up
    server_handle.abort();
    unsafe { env::remove_var("HIT_SERVER_URL"); }
}

#[tokio::test]
async fn test_send_object_to_server_failure() {
    // Set an invalid server URL that will definitely fail
    unsafe { env::set_var("HIT_SERVER_URL", "http://invalid-domain-that-does-not-exist-12345.com"); }
    
    let blob = Blob {
        content: b"Test content".to_vec(),
    };
    let obj = Object::Blob(blob);
    
    // This should fail because the server domain doesn't exist
    let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
        send_object_to_server(&obj).map_err(|e| e.to_string())
    }).await.unwrap();
    
    assert!(result.is_err(), "Expected send_object_to_server to fail with invalid domain, but it succeeded");
    
    // Clean up
    unsafe { env::remove_var("HIT_SERVER_URL"); }
}

#[tokio::test]
async fn test_concurrent_object_operations() {
    let app = create_test_app();
    
    // Create multiple objects concurrently
    let mut handles = vec![];
    
    for i in 0..10 {
        let app_clone = app.clone();
        let handle = tokio::spawn(async move {
            let content = format!("Concurrent test content {}", i);
            let blob = Blob {
                content: content.as_bytes().to_vec(),
            };
            let obj = Object::Blob(blob.clone());
            let hash = obj.hash();
            
            // Store the object
            let serialized = bincode::serialize(&obj).unwrap();
            let store_req = Request::builder()
                .method("PUT")
                .uri(format!("/objects/{}", hash))
                .header("Content-Type", "application/octet-stream")
                .body(Body::from(serialized))
                .unwrap();
            
            let store_resp = app_clone.clone().oneshot(store_req).await.unwrap();
            assert_eq!(store_resp.status(), StatusCode::OK);
            
            // Retrieve the object
            let get_req = Request::builder()
                .method("GET")
                .uri(format!("/objects/{}", hash))
                .body(Body::empty())
                .unwrap();
            
            let get_resp = app_clone.oneshot(get_req).await.unwrap();
            assert_eq!(get_resp.status(), StatusCode::OK);
            
            let body = axum::body::to_bytes(get_resp.into_body(), usize::MAX).await.unwrap();
            let retrieved_obj: Object = bincode::deserialize(&body).unwrap();
            
            match retrieved_obj {
                Object::Blob(retrieved_blob) => {
                    assert_eq!(retrieved_blob.content, blob.content);
                }
                _ => panic!("Expected Blob object"),
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all operations to complete
    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn test_object_storage_with_large_content() {
    let app = create_test_app();
    
    // Create a large object (1MB)
    let large_content: Vec<u8> = (0..1_000_000).map(|i| (i % 256) as u8).collect();
    let blob = Blob { content: large_content.clone() };
    let obj = Object::Blob(blob);
    let hash = obj.hash();
    
    // Serialize and store the object
    let serialized = bincode::serialize(&obj).unwrap();
    let store_req = Request::builder()
        .method("PUT")
        .uri(format!("/objects/{}", hash))
        .header("Content-Type", "application/octet-stream")
        .body(Body::from(serialized))
        .unwrap();
    
    let store_resp = app.clone().oneshot(store_req).await.unwrap();
    assert_eq!(store_resp.status(), StatusCode::OK);
    
    // Retrieve the object
    let get_req = Request::builder()
        .method("GET")
        .uri(format!("/objects/{}", hash))
        .body(Body::empty())
        .unwrap();
    
    let get_resp = app.oneshot(get_req).await.unwrap();
    assert_eq!(get_resp.status(), StatusCode::OK);
    
    // Verify the content matches
    let body = axum::body::to_bytes(get_resp.into_body(), usize::MAX).await.unwrap();
    let retrieved_obj: Object = bincode::deserialize(&body).unwrap();
    
    match retrieved_obj {
        Object::Blob(retrieved_blob) => {
            assert_eq!(retrieved_blob.content, large_content);
        }
        _ => panic!("Expected Blob object"),
    }
}

#[tokio::test]
async fn test_object_deduplication() {
    let app = create_test_app();
    
    // Create two identical objects
    let content = b"Duplicate content test".to_vec();
    let blob1 = Blob { content: content.clone() };
    let blob2 = Blob { content: content.clone() };
    let obj1 = Object::Blob(blob1);
    let obj2 = Object::Blob(blob2);
    
    // They should have the same hash
    assert_eq!(obj1.hash(), obj2.hash());
    let hash = obj1.hash();
    
    // Store the first object
    let serialized1 = bincode::serialize(&obj1).unwrap();
    let store_req1 = Request::builder()
        .method("PUT")
        .uri(format!("/objects/{}", hash))
        .header("Content-Type", "application/octet-stream")
        .body(Body::from(serialized1))
        .unwrap();
    
    let store_resp1 = app.clone().oneshot(store_req1).await.unwrap();
    assert_eq!(store_resp1.status(), StatusCode::OK);
    
    // Store the second identical object (should succeed)
    let serialized2 = bincode::serialize(&obj2).unwrap();
    let store_req2 = Request::builder()
        .method("PUT")
        .uri(format!("/objects/{}", hash))
        .header("Content-Type", "application/octet-stream")
        .body(Body::from(serialized2))
        .unwrap();
    
    let store_resp2 = app.clone().oneshot(store_req2).await.unwrap();
    assert_eq!(store_resp2.status(), StatusCode::OK);
    
    // Retrieve the object - should work fine
    let get_req = Request::builder()
        .method("GET")
        .uri(format!("/objects/{}", hash))
        .body(Body::empty())
        .unwrap();
    
    let get_resp = app.oneshot(get_req).await.unwrap();
    assert_eq!(get_resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_object_hash_consistency() {
    // Test that the same content always produces the same hash
    let content = b"Hash consistency test".to_vec();
    
    let blob1 = Blob { content: content.clone() };
    let blob2 = Blob { content: content.clone() };
    let obj1 = Object::Blob(blob1);
    let obj2 = Object::Blob(blob2);
    
    assert_eq!(obj1.hash(), obj2.hash());
    
    // Test that different content produces different hashes
    let different_content = b"Different content".to_vec();
    let blob3 = Blob { content: different_content };
    let obj3 = Object::Blob(blob3);
    
    assert_ne!(obj1.hash(), obj3.hash());
}
