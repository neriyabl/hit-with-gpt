use serde::{Serialize, Deserialize};
use sha2::{Digest, Sha256};

/// Trait for objects that can produce a stable hash identifier.
pub trait Hashable {
    fn hash(&self) -> String;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Blob {
    pub content: Vec<u8>,
}

impl Hashable for Blob {
    fn hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(&bincode::serialize(self).expect("failed to serialize blob"));
        format!("{:x}", hasher.finalize())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TreeEntry {
    Blob { name: String, blob: Blob },
    Tree { name: String, tree: Tree },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Tree {
    pub entries: Vec<TreeEntry>,
}

impl Hashable for Tree {
    fn hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(&bincode::serialize(self).expect("failed to serialize tree"));
        format!("{:x}", hasher.finalize())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Commit {
    pub tree: Tree,
    pub message: String,
}

impl Hashable for Commit {
    fn hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(&bincode::serialize(self).expect("failed to serialize commit"));
        format!("{:x}", hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_hash_is_deterministic() {
        let blob = Blob { content: b"hello".to_vec() };
        assert_eq!(blob.hash(), blob.hash());
    }

    #[test]
    fn tree_and_commit_hash() {
        let blob = Blob { content: b"hello".to_vec() };
        let tree = Tree {
            entries: vec![TreeEntry::Blob { name: "file.txt".into(), blob }],
        };
        let commit = Commit {
            tree: tree.clone(),
            message: "init".into(),
        };
        assert!(!tree.hash().is_empty());
        assert!(!commit.hash().is_empty());
    }
}
