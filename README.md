# hit

> A next-generation, AI-powered version control system for continuous, automated collaboration.

## 🧠 What is `hit`?

`hit` is an experimental version control system, written in Rust, that reimagines how developers collaborate on code.
Instead of relying on manual commands like `commit` or `merge`, `hit` continuously monitors your files and stores meaningful changes as versioned objects — automatically.

The long-term vision includes an AI-powered server that decides which changes are ready to propagate to other collaborators, removing the need for manual version control steps.

## ✨ Key Features (in progress)

* 📡 **Real-time File Tracking** – Automatically watches for file changes
* 🤖 **AI-Guided Versioning** – Future server will decide what changes are worth syncing
* 🔁 **Parallel Development** – Supports collaborative work with fewer merge headaches
* 🔐 **Immutable Objects** – Content-addressed `Blob`, `Tree`, and `Commit` structures
* 🧠 **Zero Commit Workflow** – No staging or manual commit required

## 📦 Current Capabilities

* ✅ `hit init` – Initializes a repository with `.hit/` directory
* ✅ `hit watch` – Watches for local file changes and stores them as `Blob`s
* ✅ Core object model with SHA-256 hashing and binary serialization
* ✅ File-based object storage
* ✅ Tests for all object and storage functionality
* ✅ Basic real-time streaming via SSE on `/events`

## 🧱 Architecture

```
Developer edits code
        ↓
Local watcher → Blob/Tree → stored in .hit/objects/
        ↓
    (Future) Server AI → decides whether to propagate → syncs to others
```

## 🚧 Project Status

Early stage – foundational components are working.
Server logic and AI-based syncing are still in design.

## 🛠 Build & Run

```
cargo build --release
./target/release/hit init
./target/release/hit watch
```

## 📂 Code Structure

* `src/object.rs` – Blob / Tree / Commit + Object enum
* `src/storage.rs` – Object read/write logic
* `src/repo.rs` – Repository setup (`hit init`)
* `src/watcher.rs` – Filesystem watcher (`hit watch`)
* `main.rs` – CLI commands (`clap`)

## 🛣 Roadmap

* [x] Object model
* [x] Object storage
* [x] File watcher
* [ ] Snapshot history view
* [ ] Server sync protocol
* [ ] AI change selection engine
* [ ] Multi-user conflict management

## 🧰 Tech Stack

* Rust (2024)
* `serde`, `bincode` – binary serialization
* `sha2` – hashing
* `notify` – cross-platform file watching
* `clap` – CLI parsing


Built with ❤️ and 🦀
