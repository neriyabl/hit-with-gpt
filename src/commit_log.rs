use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::commit::Commit;

/// Append-only commit log stored on disk.
pub struct CommitLog {
    path: PathBuf,
    file: File,
}

impl CommitLog {
    /// Open the commit log for appending. The file is created if it doesn't exist.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_owned();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        Ok(Self { path, file })
    }

    /// Append a commit to the log and flush to disk.
    pub fn append(&mut self, commit: &Commit) -> io::Result<()> {
        let data = bincode::serialize(commit).map_err(to_io_err)?;
        let compressed = zstd::stream::encode_all(&data[..], 0)?;
        let len = compressed.len() as u32;
        self
            .file
            .write_all(&len.to_le_bytes())
            .map_err(|e| {
                tracing::error!("failed to write commit length: {}", e);
                e
            })?;
        self.file
            .write_all(&compressed)
            .map_err(|e| {
                tracing::error!("failed to write commit data: {}", e);
                e
            })?;
        self.file.sync_data().map_err(|e| {
            tracing::error!("failed to sync commit log: {}", e);
            e
        })?;
        Ok(())
    }

    /// Load all commits from the given path.
    pub fn load(path: impl AsRef<Path>) -> io::Result<Vec<Commit>> {
        let path = path.as_ref();
        let mut file = match File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                tracing::error!("failed to open commit log at {}: {}", path.display(), e);
                return Err(e);
            }
        };
        let mut commits = Vec::new();
        loop {
            let mut len_buf = [0u8; 4];
            if let Err(e) = file.read_exact(&mut len_buf) {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    break;
                }
                tracing::error!("failed to read commit length: {}", e);
                return Err(e);
            }
            let len = u32::from_le_bytes(len_buf) as usize;
            let mut data = vec![0u8; len];
            if let Err(e) = file.read_exact(&mut data) {
                tracing::error!("failed to read commit data: {}", e);
                return Err(e);
            }
            let decompressed = match zstd::stream::decode_all(&data[..]) {
                Ok(d) => d,
                Err(e) => {
                    tracing::error!("failed to decompress commit: {}", e);
                    return Err(e);
                }
            };
            let commit: Commit = bincode::deserialize(&decompressed).map_err(|e| {
                let err = to_io_err(e);
                tracing::error!("failed to deserialize commit: {}", err);
                err
            })?;
            commits.push(commit);
        }
        Ok(commits)
    }

    /// Path backing the commit log.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn to_io_err<E: std::fmt::Display>(e: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, format!("{}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::Change;
    use crate::commit::Commit;
    use serial_test::serial;

    fn log_path() -> &'static str { "test_commits.log" }

    fn clean() { let _ = std::fs::remove_file(log_path()); }

    #[test]
    #[serial]
    fn write_and_reload() {
        clean();
        let mut log = CommitLog::open(log_path()).unwrap();
        let commit1 = Commit { id: 1, changes: vec![Change { hash: "h1".into(), path: "p".into(), timestamp: 1 }], timestamp: 1 };
        let commit2 = Commit { id: 2, changes: vec![Change { hash: "h2".into(), path: "p".into(), timestamp: 2 }], timestamp: 2 };
        log.append(&commit1).unwrap();
        log.append(&commit2).unwrap();
        drop(log);

        let loaded = CommitLog::load(log_path()).unwrap();
        assert_eq!(loaded, vec![commit1, commit2]);
        clean();
    }
}
