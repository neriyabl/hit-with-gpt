use serde::{Deserialize, Serialize};
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

    pub fn add_commit(&self, change: Change) -> Commit {
        let mut commits = self.commits.lock().unwrap();
        let id = commits.last().map(|c| c.id + 1).unwrap_or(1);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let commit = Commit {
            id,
            changes: vec![change],
            timestamp,
        };
        commits.push(commit.clone());
        commit
    }

    pub fn all(&self) -> Vec<Commit> {
        self.commits.lock().unwrap().clone()
    }

    pub fn latest(&self) -> Option<Commit> {
        self.commits.lock().unwrap().last().cloned()
    }
}
