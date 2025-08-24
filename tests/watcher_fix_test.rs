use std::fs;
use std::path::Path;

use hit_with_gpt::watcher::handle_event;
use hit_with_gpt::object::{Blob, Object, Hashable};
use hit_with_gpt::storage::write_object;
use notify::{Event, EventKind};
use serial_test::serial;

fn clean() {
    let _ = fs::remove_dir_all(".hit");
}

#[test]
#[serial]
fn test_handles_existing_objects_correctly() {
    clean();
    
    // Create a blob and store it first
    let content = b"hello world".to_vec();
    let blob = Blob { content: content.clone() };
    let obj = Object::Blob(blob);
    let hash = obj.hash();
    
    // Pre-store the object
    write_object(&obj).expect("failed to write object");
    
    // Verify object exists
    let object_path = Path::new(".hit/objects").join(&hash);
    assert!(object_path.exists(), "object should exist before test");
    
    // Create a test file with the same content
    let test_file = Path::new("test_existing.txt");
    fs::write(test_file, &content).expect("failed to write test file");
    
    // Create a mock event for the file
    let event = Event {
        kind: EventKind::Modify(notify::event::ModifyKind::Data(notify::event::DataChange::Content)),
        paths: vec![test_file.to_path_buf()],
        attrs: Default::default(),
    };
    
    // Handle the event - this should succeed even though object already exists
    let result = handle_event(event);
    
    // Clean up
    fs::remove_file(test_file).ok();
    
    // The key test: handle_event should succeed even for existing objects
    assert!(result.is_ok(), "handle_event should succeed for existing objects");
    
    // Verify the object still exists (wasn't corrupted)
    assert!(object_path.exists(), "object should still exist after handling event");
}
