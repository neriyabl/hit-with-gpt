pub mod object;
pub mod storage;

use std::{fs::{self, File}, io::{self, Write}, path::Path};

/// Initialize a new hit repository in the given directory.
///
/// Creates the `.hit` directory with the required substructure.
pub fn init_repo<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let hit_dir = path.as_ref().join(".hit");
    if hit_dir.exists() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, "repository already initialized"));
    }

    fs::create_dir_all(hit_dir.join("objects"))?;
    fs::create_dir_all(hit_dir.join("refs/heads"))?;

    File::create(hit_dir.join("config"))?;

    let mut head = File::create(hit_dir.join("HEAD"))?;
    head.write_all(b"refs/heads/main")?;

    File::create(hit_dir.join("refs/heads/main"))?;

    Ok(())
}
