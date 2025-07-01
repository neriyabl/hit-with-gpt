use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Result, Watcher, Event};

use crate::object::{Blob, Object, Hashable};
use crate::storage::{write_object, OBJECT_DIR};

/// File suffixes that should be ignored by the watcher.
pub const IGNORED_SUFFIXES: &[&str] = &["~", ".swp", ".tmp"];

pub fn watch_and_store_changes() -> Result<()> {
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
                    eprintln!("error handling event: {}", e);
                }
            }
            Err(e) => eprintln!("watch error: {:?}", e),
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
                println!(
                    "Detected change: {} \u{2192} stored as {} (unchanged, already stored)",
                    path.display(), hash
                );
            } else {
                write_object(&obj)?;
                println!("Detected change: {} \u{2192} stored as {}", path.display(), hash);
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
