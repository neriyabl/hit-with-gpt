use hit_with_gpt::commit::CommitStore;
use hit_with_gpt::server::Change;
use serial_test::serial;

#[test]
#[serial]
fn test_duplicate_changes_are_deduplicated() {
    let store = CommitStore::default();
    
    let change = Change {
        hash: "abc123".to_string(),
        path: "test.txt".to_string(),
        timestamp: 1000,
    };
    
    // Add the same change twice
    let commit1 = store.add_commit(change.clone()).expect("first commit should succeed");
    let commit2 = store.add_commit(change.clone()).expect("second commit should succeed");
    
    // Should return the same commit (no new commit created)
    assert_eq!(commit1.id, commit2.id, "duplicate changes should return the same commit");
    
    // Verify only one commit exists
    let all_commits = store.all().expect("should get all commits");
    assert_eq!(all_commits.len(), 1, "should only have one commit for duplicate changes");
}

#[test]
#[serial]
fn test_different_changes_create_separate_commits() {
    let store = CommitStore::default();
    
    let change1 = Change {
        hash: "abc123".to_string(),
        path: "test.txt".to_string(),
        timestamp: 1000,
    };
    
    let change2 = Change {
        hash: "def456".to_string(), // Different hash
        path: "test.txt".to_string(),
        timestamp: 1001,
    };
    
    let commit1 = store.add_commit(change1).expect("first commit should succeed");
    let commit2 = store.add_commit(change2).expect("second commit should succeed");
    
    // Should create different commits
    assert_ne!(commit1.id, commit2.id, "different changes should create separate commits");
    assert_eq!(commit2.id, commit1.id + 1, "second commit should have incremented ID");
    
    // Verify two commits exist
    let all_commits = store.all().expect("should get all commits");
    assert_eq!(all_commits.len(), 2, "should have two commits for different changes");
}

#[test]
#[serial]
fn test_same_hash_different_path_creates_new_commit() {
    let store = CommitStore::default();
    
    let change1 = Change {
        hash: "abc123".to_string(),
        path: "test1.txt".to_string(),
        timestamp: 1000,
    };
    
    let change2 = Change {
        hash: "abc123".to_string(), // Same hash
        path: "test2.txt".to_string(), // Different path
        timestamp: 1001,
    };
    
    let commit1 = store.add_commit(change1).expect("first commit should succeed");
    let commit2 = store.add_commit(change2).expect("second commit should succeed");
    
    // Should create different commits (same content, different files)
    assert_ne!(commit1.id, commit2.id, "same hash different path should create separate commits");
    
    // Verify two commits exist
    let all_commits = store.all().expect("should get all commits");
    assert_eq!(all_commits.len(), 2, "should have two commits for same hash different paths");
}
