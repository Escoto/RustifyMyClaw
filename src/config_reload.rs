use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use notify::{Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::config;

#[cfg(test)]
#[path = "tests/config_reload_test.rs"]
mod tests;

/// Watch `path` for modifications and call `on_reload` with the new config on each change.
///
/// Events are debounced: after the first modify event arrives, the function waits 300 ms
/// and drains any additional pending events before reloading. This avoids redundant
/// reloads caused by editors that write files in multiple steps.
///
/// Returns when `shutdown` is cancelled.
pub async fn watch(
    path: PathBuf,
    shutdown: CancellationToken,
    on_reload: impl Fn(config::AppConfig) + Send + 'static,
) -> Result<()> {
    let (event_tx, mut event_rx) = mpsc::channel::<notify::Result<Event>>(16);

    // `RecommendedWatcher` callbacks are sync (called from a background thread by notify).
    // Bridge to the async world via `blocking_send`, which is safe from non-async contexts.
    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<Event>| {
            let _ = event_tx.blocking_send(res);
        },
        NotifyConfig::default(),
    )?;

    watcher.watch(&path, RecursiveMode::NonRecursive)?;
    info!(path = %path.display(), "config file watch started");

    loop {
        tokio::select! {
            event = event_rx.recv() => {
                let Some(event) = event else {
                    info!("config watcher channel closed");
                    break;
                };
                match event {
                    Ok(e) if e.kind.is_modify() => {
                        // Debounce: wait 300 ms, then drain pending events.
                        tokio::time::sleep(Duration::from_millis(300)).await;
                        while event_rx.try_recv().is_ok() {}

                        match config::load_from_path(&path) {
                            Ok(new_config) => {
                                info!("config reloaded from {}", path.display());
                                on_reload(new_config);
                            }
                            Err(e) => {
                                warn!(error = ?e, "config reload failed — keeping current config");
                            }
                        }
                    }
                    Ok(_) => {} // create / delete / metadata / access — ignore
                    Err(e) => warn!(error = ?e, "config watcher error"),
                }
            }
            _ = shutdown.cancelled() => {
                info!("config watcher shutting down");
                break;
            }
        }
    }

    Ok(())
}
