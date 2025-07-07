use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tracing::info;

/// Initialize a new hit repository in the current directory.
///
/// Creates the `.hit` directory along with required subdirectories and files.
/// Reinitializing an existing repository does not error.
pub fn init() -> std::io::Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let hit_dir = cwd.join(".hit");

    if hit_dir.exists() {
        info!(path = %hit_dir.display(), "Reinitialized existing hit repository");
    } else {
        info!(path = %hit_dir.display(), "Initialized empty hit repository");
    }

    fs::create_dir_all(hit_dir.join("objects"))?;
    fs::create_dir_all(hit_dir.join("refs").join("heads"))?;

    let config_path = hit_dir.join("config");
    if !config_path.exists() {
        File::create(&config_path)?;
    }

    let head_path = hit_dir.join("HEAD");
    if !head_path.exists() {
        let mut head = File::create(&head_path)?;
        head.write_all(b"refs/heads/main")?;
    }

    let main_ref = hit_dir.join("refs").join("heads").join("main");
    if !main_ref.exists() {
        File::create(&main_ref)?;
    }

    Ok(hit_dir)
}
