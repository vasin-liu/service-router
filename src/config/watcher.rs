use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tracing::{error, info, warn};

use crate::config::loader::load_config;
use crate::config::model::AppConfig;

/// Debounce delay before re-loading the config after a file change.
const DEBOUNCE_DELAY: Duration = Duration::from_millis(500);

/// Start a background file-watcher that reloads `config_path` on any write
/// event and stores the new config into `config_slot`.
///
/// Returns the watcher handle; dropping it stops watching.
pub fn start_config_watcher(
    config_path: PathBuf,
    config_slot: Arc<ArcSwap<AppConfig>>,
) -> notify::Result<RecommendedWatcher> {
    let path_for_closure = config_path.clone();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        match res {
            Ok(event) => handle_event(&event, &path_for_closure, &config_slot),
            Err(e) => warn!("Config watcher error: {}", e),
        }
    })?;

    // Watch the parent directory so renames (vim-style saves) are detected.
    let parent = config_path.parent().unwrap_or_else(|| std::path::Path::new("."));
    watcher.watch(parent, RecursiveMode::NonRecursive)?;

    info!("Config hot-reload watching: {}", config_path.display());
    Ok(watcher)
}

fn handle_event(event: &Event, config_path: &PathBuf, config_slot: &Arc<ArcSwap<AppConfig>>) {
    // Only react to modify / create events that concern our specific file.
    let is_relevant = matches!(
        event.kind,
        EventKind::Modify(_) | EventKind::Create(_)
    ) && event.paths.iter().any(|p| p == config_path);

    if !is_relevant {
        return;
    }

    // Short sleep to let the write complete (debounce).
    std::thread::sleep(DEBOUNCE_DELAY);

    match load_config(config_path) {
        Ok(new_config) => {
            info!("Config reloaded from {}", config_path.display());
            config_slot.store(Arc::new(new_config));
        }
        Err(e) => {
            error!("Failed to reload config (keeping current): {}", e);
        }
    }
}
