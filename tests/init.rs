use std::fs;

use hit_with_gpt::repo;

fn clean() {
    let _ = fs::remove_dir_all(".hit");
}

#[test]
fn creates_repository_structure() {
    clean();
    repo::init().unwrap();
    assert!(fs::metadata(".hit").unwrap().is_dir());
    assert!(fs::metadata(".hit/objects").unwrap().is_dir());
    assert!(fs::metadata(".hit/refs/heads").unwrap().is_dir());
    assert!(fs::metadata(".hit/refs/heads/main").unwrap().is_file());
    assert!(fs::metadata(".hit/HEAD").unwrap().is_file());
}

#[test]
fn init_is_idempotent() {
    clean();
    repo::init().unwrap();
    repo::init().unwrap();
    assert!(fs::metadata(".hit").unwrap().is_dir());
}
