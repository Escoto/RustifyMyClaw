use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::backend::{self, CliBackend};
use crate::channel::{self, ChannelProvider};
use crate::config::{self, OutputConfig};
use crate::config_reload;
use crate::rate_limit::RateLimiter;
use crate::router::Router;
use crate::session::SessionStore;
use crate::types::{InboundMessage, WorkspaceHandle};

/// Bundles the three collections produced by workspace initialisation.
pub(crate) struct WorkspaceSetup {
    pub backends: HashMap<String, Arc<dyn CliBackend>>,
    pub workspaces: HashMap<String, WorkspaceHandle>,
    pub providers: Vec<Arc<dyn ChannelProvider>>,
}

/// Snapshot + mutable wrapper returned by [`build_rate_limiter`].
pub(crate) type SharedRateLimiter = Arc<RwLock<Option<Arc<RateLimiter>>>>;

/// Spawn the OS signal listener. Cancels `shutdown` on first SIGTERM or Ctrl+C.
pub(crate) fn spawn_signal_handler(shutdown: CancellationToken) {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm =
                signal(SignalKind::terminate()).expect("failed to register SIGTERM handler");
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = sigterm.recv() => {}
            }
        }
        #[cfg(not(unix))]
        {
            tokio::signal::ctrl_c().await.ok();
        }
        info!("shutdown signal received — initiating graceful shutdown");
        shutdown.cancel();
    });
}

/// Build the optional rate limiter and its hot-reload wrapper.
///
/// Returns `(snapshot, shared)` — the snapshot is passed to the [`Router`], while
/// `shared` is held by the config watcher for in-flight updates.
pub(crate) fn build_rate_limiter(
    config: &config::AppConfig,
) -> (Option<Arc<RateLimiter>>, SharedRateLimiter) {
    let rate_limiter: Option<Arc<RateLimiter>> = config.limits.as_ref().map(|l| {
        Arc::new(RateLimiter::new(
            l.max_requests,
            Duration::from_secs(l.window_seconds),
        ))
    });
    let shared = Arc::new(RwLock::new(rate_limiter.clone()));
    (rate_limiter, shared)
}

/// Iterate workspace configs and construct backends, workspace handles, and channel providers.
pub(crate) async fn build_workspaces(
    config: &config::AppConfig,
    global_output: &Arc<OutputConfig>,
) -> Result<WorkspaceSetup> {
    let mut backends: HashMap<String, Arc<dyn CliBackend>> = HashMap::new();
    let mut workspaces: HashMap<String, WorkspaceHandle> = HashMap::new();
    let mut providers: Vec<Arc<dyn ChannelProvider>> = Vec::new();

    for ws_config in &config.workspaces {
        let workspace_handle = WorkspaceHandle {
            name: ws_config.name.clone(),
            directory: ws_config.directory.clone(),
            backend: ws_config.backend.clone(),
            timeout: ws_config.timeout_seconds.map(Duration::from_secs),
        };

        workspaces.insert(ws_config.name.clone(), workspace_handle.clone());

        // Wrap in Arc<RwLock<>> so the /use command can swap the workspace at runtime.
        let workspace = Arc::new(RwLock::new(workspace_handle));

        // Register backend if not already present.
        if !backends.contains_key(&ws_config.backend) {
            let b = backend::build(&ws_config.backend)?;
            backends.insert(ws_config.backend.clone(), Arc::from(b));
        }

        for ch_config in &ws_config.channels {
            let provider = channel::build(
                ch_config,
                &ws_config.name,
                Arc::clone(&workspace),
                global_output,
            )
            .await?;
            providers.push(provider);
        }
    }

    Ok(WorkspaceSetup {
        backends,
        workspaces,
        providers,
    })
}

/// Create the [`Router`] and spawn it as a background task.
pub(crate) fn spawn_router(
    session_store: Arc<RwLock<SessionStore>>,
    backends: HashMap<String, Arc<dyn CliBackend>>,
    workspaces: HashMap<String, WorkspaceHandle>,
    rate_limiter: Option<Arc<RateLimiter>>,
    rx: mpsc::Receiver<InboundMessage>,
    shutdown: CancellationToken,
) -> JoinHandle<()> {
    let router = Arc::new(Router::new(
        session_store,
        backends,
        workspaces,
        rate_limiter,
    ));
    tokio::spawn(async move { router.run(rx, shutdown).await })
}

/// Spawn every channel provider as a background task.
///
/// Takes `tx` by value — each provider receives its own clone, and the original is
/// dropped when this function returns, so the router will see channel closure once
/// all providers stop.
pub(crate) fn spawn_providers(
    providers: Vec<Arc<dyn ChannelProvider>>,
    tx: mpsc::Sender<InboundMessage>,
    shutdown: CancellationToken,
) -> Vec<JoinHandle<()>> {
    providers
        .into_iter()
        .map(|provider| {
            let tx = tx.clone();
            let shutdown = shutdown.clone();
            let provider_ref = Arc::clone(&provider);
            tokio::spawn(async move {
                if let Err(e) = provider_ref.start(tx, provider, shutdown).await {
                    tracing::error!(error = ?e, "channel provider crashed");
                }
            })
        })
        .collect()
}

/// Spawn the config hot-reload file watcher.
///
/// `config_baseline` is consumed by the reload diff closure.
pub(crate) fn spawn_config_watcher(
    config_baseline: config::AppConfig,
    shutdown: CancellationToken,
    rate_limiter_shared: SharedRateLimiter,
) {
    let config_path = config::dirs_path();
    tokio::spawn(async move {
        if let Err(e) = config_reload::watch(
            config_path,
            shutdown,
            move |new_config: config::AppConfig| {
                config::diff_reload(&config_baseline, &new_config);

                // Hot-reload: update rate limiter if limits changed.
                let new_limiter = new_config.limits.as_ref().map(|l| {
                    Arc::new(RateLimiter::new(
                        l.max_requests,
                        Duration::from_secs(l.window_seconds),
                    ))
                });
                // Use blocking write since this closure is sync.
                if let Ok(mut guard) = rate_limiter_shared.try_write() {
                    *guard = new_limiter;
                }
            },
        )
        .await
        {
            tracing::warn!(error = ?e, "config watcher failed");
        }
    });
}

/// Wait for all provider tasks, then the router, then log completion.
pub(crate) async fn await_shutdown(
    provider_handles: Vec<JoinHandle<()>>,
    router_handle: JoinHandle<()>,
) {
    for handle in provider_handles {
        let _ = handle.await;
    }
    let _ = router_handle.await;
    info!("rustifymyclaw shutdown complete");
}
