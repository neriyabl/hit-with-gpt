use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::server::Change;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Commit {
    pub id: u64,
    pub changes: Vec<Change>,
    pub timestamp: u64,
}

#[derive(Clone, Default)]
pub struct CommitStore {
    pub commits: Arc<Mutex<Vec<Commit>>>,
}

impl CommitStore {
    pub fn new() -> Self {
        Self::default()
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
}
