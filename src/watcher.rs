use std::path::Path;
use std::sync::mpsc::channel;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, error::Error};
use tracing::{error, info, warn};

use reqwest::blocking::Client;
use serde_json::json;

use notify::{Event, RecommendedWatcher, RecursiveMode, Result as NotifyResult, Watcher};

use crate::object::{Blob, Object, Hashable};
use crate::storage::{write_object, OBJECT_DIR};

/// File suffixes that should be ignored by the watcher.
pub const IGNORED_SUFFIXES: &[&str] = &["~", ".swp", ".tmp"];

/// Send a newly detected change to the configured server.
pub fn send_change_to_server(hash: &str, path: &Path) -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let base = env::var("HIT_SERVER_URL").unwrap_or_else(|_| "http://localhost:8888".into());
    let url = format!("{}/changes", base.trim_end_matches('/'));
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let body = json!({
        "hash": hash,
        "path": path.to_string_lossy(),
        "timestamp": timestamp,
    });
    let resp = client.post(&url).json(&body).send()?;
    if !resp.status().is_success() {
        return Err(format!("server responded with status {}", resp.status()).into());
    }
    info!(hash, path = %path.display(), status = %resp.status(), "Sent change to server");
    Ok(())
}

pub fn watch_and_store_changes() -> NotifyResult<()> {
    let (tx, rx) = channel();

    let mut watcher = RecommendedWatcher::new(
        tx,
        notify::Config::default()
            .with_poll_interval(Duration::from_secs(1))
            .with_compare_contents(true),
    )?;

    watcher.watch(Path::new("."), RecursiveMode::Recursive)?;

    for res in rx {
        match res {
            Ok(event) => {
                if let Err(e) = handle_event(event) {
                    error!(%e, "error handling event");
                }
            }
            Err(e) => error!(?e, "watch error"),
        }
    }
    Ok(())
}

/// Handle a single notify [`Event`].
///
/// This function is public so it can be unit tested without running the
/// watcher loop.
pub fn handle_event(event: Event) -> std::io::Result<()> {
    for path in event.paths {
        if should_ignore(&path) {
            continue;
        }
        if path.is_file() {
            let content = std::fs::read(&path)?;
            let blob = Blob { content };
            let obj = Object::Blob(blob);
            let hash = obj.hash();
            let object_path = Path::new(OBJECT_DIR).join(&hash);
            if object_path.exists() {
                info!(path = %path.display(), hash, "Detected change (already stored)");
            } else {
                write_object(&obj)?;
                info!(path = %path.display(), hash, "Detected change stored");
                if let Err(e) = send_change_to_server(&hash, &path) {
                    warn!(%e, "failed to send change to server");
                }
            }
        }
    }
    Ok(())
}

fn should_ignore(path: &Path) -> bool {
    if path.components().any(|c| c.as_os_str() == ".hit") {
        return true;
    }
    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
        for suffix in IGNORED_SUFFIXES {
            if name.ends_with(suffix) {
                return true;
            }
        }
    }
    false
}
