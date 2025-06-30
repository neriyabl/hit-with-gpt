# hit

A next-generation, AI-powered automatic version control system for real-time collaboration.

## Overview

`hit` is not a Git clone. It removes the need for manual `add` or `commit` commands. A local watcher continuously tracks file changes, while an AI backend decides what and when to version and share with others.

## Features

- ✅ Object model (`Blob`, `Tree`, `Commit`)
- ✅ File-based object storage
- ✅ `hit init` – initialize a repository
- ✅ `hit watch` – local real-time file change monitoring

## Architecture

- Local watcher detects file changes and stores them as immutable objects (`Blob`, `Tree`, `Commit`).
- A central server receives change notifications and decides when to propagate them.
- Future plans: IDE integration, semantic diffing, and conflict-free merges.

## Roadmap

- [x] Object model
- [x] Object storage
- [x] Repository initialization
- [x] Basic file watcher
- [ ] Server communication
- [ ] AI commit decision engine
- [ ] Real-time collaboration protocol
- [ ] Version tree viewer (CLI or UI)

## Getting Started

Install via `cargo install` or build from source:

```bash
cargo install hit
# or
cargo build --release
```

Initialize a repository and start watching:

```bash
hit init
hit watch
```

## Vision

Eventually `hit` will support multi-developer collaboration without the overhead of merges, commits, or branches. AI will infer developer intent and handle versioning autonomously.

## Contributing

`hit` is pre-alpha. Contributions are welcome, but expect rapid changes.
