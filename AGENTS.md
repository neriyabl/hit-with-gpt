**AGENTS.md**

---

# AGENTS.md

## 🔍 Project Overview

This project, called **hit**, is a version control system written in Rust.
It aims to be a Git alternative with support for parallel development and automatic AI-powered commits.

Core object types are:

* `Blob`: stores file content
* `Tree`: stores directory structure
* `Commit`: stores snapshots of the repository
* `Object`: an enum of Blob / Tree / Commit

Objects are serialized with `bincode` and hashed using SHA-256.
They are stored on disk under `.hit/objects/<hash>`.

---

## 🧱 Code Layout

* `src/object.rs`: defines the core object model and the `Hashable` trait.
* `src/storage.rs`: handles saving and loading objects from disk.
* `src/main.rs`: (to be implemented) will contain the CLI entry point.
* `tests/`: inline module tests using `#[cfg(test)]`.

---

## 🛠 Build Instructions

```
cargo build --release
```

## 🧪 Running Tests

```
cargo test
```

---

## 🧰 Dependencies

This project uses the following crates:

* `serde` + `bincode` for serialization
* `sha2` for hashing
* `clap` for CLI (to be added)
* `std::fs` and `std::io` for file operations

---

## 🎯 Development Conventions

* Objects are hashed using SHA-256 over the `bincode`-encoded struct
* Object hashes are stable and deterministic
* All structs implement `Clone`, `Debug`, `Serialize`, `Deserialize`, and `PartialEq`
* Tests must round-trip: serialize → write → read → deserialize → compare
* Unsafe Rust is not allowed

---

## 🚀 Planned CLI Commands

* `hit init` – initialize a new repository
* `hit add <file>` – add file(s) to staging
* `hit commit` – commit staged changes
* `hit status` – show working directory status
* `hit log` – show commit history
* `hit push` / `hit pull` – synchronize with central server (future)
