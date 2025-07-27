use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::commit_log::CommitLog;

use crate::server::Change;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Commit {
    pub id: u64,
    pub changes: Vec<Change>,
    pub timestamp: u64,
}

#[derive(Clone)]
pub struct CommitStore {
    pub commits: Arc<Mutex<Vec<Commit>>>,
    log: Option<Arc<Mutex<CommitLog>>>,
}

impl Default for CommitStore {
    fn default() -> Self {
        Self {
            commits: Arc::new(Mutex::new(Vec::new())),
            log: None,
        }
    }
}

impl CommitStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a commit store backed by a commit log at the given path.
    pub fn with_log(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let commits = CommitLog::load(&path).map_err(|e| {
            tracing::error!("failed to load commit log: {}", e);
            e
        })?;
        let log = CommitLog::open(path).map_err(|e| {
            tracing::error!("failed to open commit log: {}", e);
            e
        })?;
        Ok(Self {
            commits: Arc::new(Mutex::new(commits)),
            log: Some(Arc::new(Mutex::new(log))),
        })
    }

    pub fn add_commit(&self, change: Change) -> Result<Commit, Box<dyn Error>> {
        let mut commits = self.commits.lock().map_err(|_| "Lock poisoned")?;
        let id = commits.last().map(|c| c.id + 1).unwrap_or(1);
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let commit = Commit {
            id,
            changes: vec![change],
            timestamp,
        };
        if let Some(log) = &self.log {
            let mut log = log.lock().map_err(|_| "Lock poisoned")?;
            if let Err(e) = log.append(&commit) {
                tracing::error!("failed to append commit to log: {}", e);
                return Err(Box::new(e));
            }
        }
        commits.push(commit.clone());
        Ok(commit)
    }

    pub fn all(&self) -> Result<Vec<Commit>, Box<dyn Error>> {
        Ok(self.commits.lock().map_err(|_| "Lock poisoned")?.clone())
    }

    pub fn latest(&self) -> Result<Option<Commit>, Box<dyn Error>> {
        Ok(self
            .commits
            .lock()
            .map_err(|_| "Lock poisoned")?
            .last()
            .cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::Change;

    #[test]
    fn error_on_poisoned_lock() {
        let store = CommitStore::default();
        // Poison the mutex
        {
            let store2 = store.clone();
            std::thread::spawn(move || {
                let _guard = store2.commits.lock().unwrap();
                panic!("boom");
            })
            .join()
            .ok();
        }
        let change = Change {
            hash: "h".into(),
            path: "p".into(),
            timestamp: 0,
        };
        let res = store.add_commit(change);
        assert!(res.is_err());
    }

    #[test]
    fn no_memory_update_on_log_failure() {
        let mut store = CommitStore::default();
        store.log = Some(Arc::new(Mutex::new(CommitLog::open("/dev/full").unwrap())));
        let change = Change {
            hash: "h".into(),
            path: "p".into(),
            timestamp: 0,
        };
        let res = store.add_commit(change);
        assert!(res.is_err());
        assert_eq!(store.commits.lock().unwrap().len(), 0);
    }
}
