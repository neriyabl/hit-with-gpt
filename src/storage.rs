use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::object::{Object, Hashable};

/// Directory where objects are stored.
pub const OBJECT_DIR: &str = ".hit/objects";

pub fn write_object(obj: &Object) -> std::io::Result<String> {
    fs::create_dir_all(OBJECT_DIR)?;
    let hash = obj.hash();
    let path: PathBuf = Path::new(OBJECT_DIR).join(&hash);
    if !path.exists() {
        let bytes = bincode::serialize(obj)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let mut file = File::create(&path)?;
        file.write_all(&bytes)?;
    }
    Ok(hash)
}

pub fn read_object(hash: &str) -> std::io::Result<Object> {
    let path: PathBuf = Path::new(OBJECT_DIR).join(hash);
    let mut file = File::open(&path)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    let obj: Object = bincode::deserialize(&buf)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    Ok(obj)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::{Blob, Tree, TreeEntry, Commit};
    use std::fs;
    use serial_test::serial;

    fn clean() {
        let _ = fs::remove_dir_all(".hit");
    }

    #[test]
    #[serial]
    fn blob_roundtrip() {
        clean();
        let blob = Blob { content: b"hello".to_vec() };
        let obj = Object::Blob(blob.clone());
        let hash = obj.hash();
        let written = write_object(&obj).unwrap();
        assert_eq!(hash, written);
        let read = read_object(&hash).unwrap();
        assert_eq!(obj, read);
    }

    #[test]
    #[serial]
    fn tree_roundtrip() {
        clean();
        let blob = Blob { content: b"hello".to_vec() };
        let tree = Tree { entries: vec![TreeEntry::Blob { name: "file".into(), blob }] };
        let obj = Object::Tree(tree.clone());
        let hash = obj.hash();
        let written = write_object(&obj).unwrap();
        assert_eq!(hash, written);
        let read = read_object(&hash).unwrap();
        assert_eq!(obj, read);
    }

    #[test]
    #[serial]
    fn commit_roundtrip() {
        clean();
        let blob = Blob { content: b"hello".to_vec() };
        let tree = Tree { entries: vec![TreeEntry::Blob { name: "file".into(), blob }] };
        let commit = Commit { tree: tree.clone(), message: "msg".into() };
        let obj = Object::Commit(commit.clone());
        let hash = obj.hash();
        let written = write_object(&obj).unwrap();
        assert_eq!(hash, written);
        let read = read_object(&hash).unwrap();
        assert_eq!(obj, read);
    }
}
