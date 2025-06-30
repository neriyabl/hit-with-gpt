use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn init_creates_directories() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("hit").unwrap()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();

    let hit_dir = dir.path().join(".hit");
    assert!(hit_dir.is_dir());
    assert!(hit_dir.join("objects").is_dir());
    assert!(hit_dir.join("refs/heads").is_dir());
    assert!(hit_dir.join("HEAD").is_file());
    assert!(hit_dir.join("config").is_file());
}

#[test]
fn init_is_idempotent() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("hit").unwrap()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();

    Command::cargo_bin("hit").unwrap()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .failure();
}
