use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::info;

mod backend;
mod channel;
mod command;
mod config;
mod config_reload;
mod executor;
mod formatter;
mod rate_limit;
mod router;
mod security;
mod session;
mod types;

use crate::channel::slack::SlackProvider;
use crate::channel::telegram::TelegramProvider;
use crate::channel::whatsapp::WhatsAppProvider;
use crate::channel::ChannelProvider;
use crate::rate_limit::RateLimiter;
use crate::router::Router;
use crate::security::SecurityGate;
use crate::session::SessionStore;
use crate::types::WorkspaceHandle;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let app_config = config::load().context("failed to load configuration")?;

    // Root cancellation token — cancelled when a shutdown signal arrives.
    let shutdown = CancellationToken::new();

    // Spawn the OS signal listener. Cancels `shutdown` on first SIGTERM or Ctrl+C.
    {
        let shutdown = shutdown.clone();
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

    let global_output = Arc::new(app_config.output.clone());

    // Optional rate limiter built from the top-level `limits` config block.
    let rate_limiter: Option<Arc<RateLimiter>> = app_config.limits.as_ref().map(|l| {
        Arc::new(RateLimiter::new(
            l.max_requests,
            Duration::from_secs(l.window_seconds),
        ))
    });

    // Shared rate limiter reference for hot-reload updates.
    let rate_limiter_shared: Arc<RwLock<Option<Arc<RateLimiter>>>> =
        Arc::new(RwLock::new(rate_limiter.clone()));

    // One session store shared across all workspaces.
    let session_store = Arc::new(RwLock::new(SessionStore::new()));

    // One inbound message channel — all providers funnel into the single router.
    let (tx, rx) = mpsc::channel(64);

    // Collect all backend implementations keyed by name for the router registry.
    let mut backends: HashMap<String, Arc<dyn backend::CliBackend>> = HashMap::new();

    // All workspace handles keyed by name — used by `/use` for runtime switching.
    let mut available_workspaces: HashMap<String, WorkspaceHandle> = HashMap::new();

    let mut provider_arcs: Vec<Arc<dyn ChannelProvider>> = Vec::new();

    for ws_config in &app_config.workspaces {
        let workspace_handle = WorkspaceHandle {
            name: ws_config.name.clone(),
            directory: ws_config.directory.clone(),
            backend: ws_config.backend.clone(),
            timeout: ws_config.timeout_seconds.map(Duration::from_secs),
        };

        available_workspaces.insert(ws_config.name.clone(), workspace_handle.clone());

        // Wrap in Arc<RwLock<>> so the /use command can swap the workspace at runtime.
        let workspace = Arc::new(RwLock::new(workspace_handle));

        // Register backend if not already present.
        if !backends.contains_key(&ws_config.backend) {
            let b = backend::build(&ws_config.backend)?;
            backends.insert(ws_config.backend.clone(), Arc::from(b));
        }

        for ch_config in &ws_config.channels {
            match ch_config.kind.as_str() {
                "telegram" => {
                    // Build a temporary provider just to call resolve_users (no clone cost).
                    let tmp = TelegramProvider::new(
                        ch_config.token.clone(),
                        SecurityGate::new(Default::default()),
                        Arc::clone(&workspace),
                        Arc::clone(&global_output),
                    );
                    let resolved = tmp
                        .resolve_users(&ch_config.allowed_users)
                        .await
                        .with_context(|| {
                            format!(
                                "workspace `{}`: failed to resolve telegram allowed_users",
                                ws_config.name
                            )
                        })?;

                    let gate = SecurityGate::new(resolved);
                    let effective_output =
                        Arc::new(config::effective_output_config(&global_output, ch_config));
                    let provider: Arc<dyn ChannelProvider> = Arc::new(TelegramProvider::new(
                        ch_config.token.clone(),
                        gate,
                        Arc::clone(&workspace),
                        effective_output,
                    ));

                    info!(
                        workspace = ws_config.name,
                        bot_name = ch_config.bot_name.as_deref().unwrap_or("(unnamed)"),
                        "telegram channel registered"
                    );

                    provider_arcs.push(provider);
                }
                "whatsapp" => {
                    let phone_number_id = ch_config.phone_number_id.clone().with_context(|| {
                        format!(
                            "workspace `{}`: whatsapp channel requires `phone_number_id`",
                            ws_config.name
                        )
                    })?;
                    let verify_token = ch_config.verify_token.clone().unwrap_or_default();

                    let tmp = WhatsAppProvider::new(
                        ch_config.token.clone(),
                        phone_number_id.clone(),
                        ch_config.webhook_port,
                        verify_token.clone(),
                        SecurityGate::new(Default::default()),
                        Arc::clone(&workspace),
                        Arc::clone(&global_output),
                    );
                    let resolved = tmp
                        .resolve_users(&ch_config.allowed_users)
                        .await
                        .with_context(|| {
                            format!(
                                "workspace `{}`: failed to resolve whatsapp allowed_users",
                                ws_config.name
                            )
                        })?;

                    let gate = SecurityGate::new(resolved);
                    let effective_output =
                        Arc::new(config::effective_output_config(&global_output, ch_config));
                    let provider: Arc<dyn ChannelProvider> = Arc::new(WhatsAppProvider::new(
                        ch_config.token.clone(),
                        phone_number_id,
                        ch_config.webhook_port,
                        verify_token,
                        gate,
                        Arc::clone(&workspace),
                        effective_output,
                    ));

                    info!(
                        workspace = ws_config.name,
                        port = ch_config.webhook_port.unwrap_or(8080),
                        "whatsapp channel registered"
                    );

                    provider_arcs.push(provider);
                }

                "slack" => {
                    let app_token = ch_config.app_token.clone().with_context(|| {
                        format!(
                            "workspace `{}`: slack channel requires `app_token` (xapp-…)",
                            ws_config.name
                        )
                    })?;
                    let use_threads = ch_config.use_threads.unwrap_or(false);

                    let tmp = SlackProvider::new(
                        ch_config.token.clone(),
                        app_token.clone(),
                        use_threads,
                        SecurityGate::new(Default::default()),
                        Arc::clone(&workspace),
                        Arc::clone(&global_output),
                    );
                    let resolved = tmp
                        .resolve_users(&ch_config.allowed_users)
                        .await
                        .with_context(|| {
                            format!(
                                "workspace `{}`: failed to resolve slack allowed_users",
                                ws_config.name
                            )
                        })?;

                    let gate = SecurityGate::new(resolved);
                    let effective_output =
                        Arc::new(config::effective_output_config(&global_output, ch_config));
                    let provider: Arc<dyn ChannelProvider> = Arc::new(SlackProvider::new(
                        ch_config.token.clone(),
                        app_token,
                        use_threads,
                        gate,
                        Arc::clone(&workspace),
                        effective_output,
                    ));

                    info!(
                        workspace = ws_config.name,
                        bot_name = ch_config.bot_name.as_deref().unwrap_or("(unnamed)"),
                        "slack channel registered"
                    );

                    provider_arcs.push(provider);
                }

                other => {
                    bail!("channel kind `{other}` is not implemented");
                }
            }
        }
    }

    // Snapshot the config for the hot-reload diff baseline.
    let config_for_reload = app_config.clone();

    // Start the router.
    let router = Arc::new(Router::new(
        Arc::clone(&session_store),
        backends,
        available_workspaces,
        rate_limiter,
    ));
    let router_handle = tokio::spawn({
        let router = Arc::clone(&router);
        let shutdown = shutdown.clone();
        async move { router.run(rx, shutdown).await }
    });

    // Start all channel providers.
    let mut handles = Vec::new();
    for provider in provider_arcs {
        let tx_clone = tx.clone();
        let provider_clone = Arc::clone(&provider);
        let shutdown_clone = shutdown.clone();
        handles.push(tokio::spawn(async move {
            if let Err(e) = provider_clone
                .start(tx_clone, provider, shutdown_clone)
                .await
            {
                tracing::error!(error = ?e, "channel provider crashed");
            }
        }));
    }

    // Drop the original sender so the router exits when all providers have stopped.
    drop(tx);

    // Spawn config hot-reload watcher.
    {
        let config_path = config::dirs_path();
        let shutdown_clone = shutdown.clone();
        let rate_limiter_ref = Arc::clone(&rate_limiter_shared);
        tokio::spawn(async move {
            if let Err(e) = config_reload::watch(
                config_path,
                shutdown_clone,
                move |new_config: config::AppConfig| {
                    config::diff_reload(&config_for_reload, &new_config);

                    // Hot-reload: update rate limiter if limits changed.
                    let new_limiter = new_config.limits.as_ref().map(|l| {
                        Arc::new(RateLimiter::new(
                            l.max_requests,
                            Duration::from_secs(l.window_seconds),
                        ))
                    });
                    // Use blocking write since this closure is sync.
                    if let Ok(mut guard) = rate_limiter_ref.try_write() {
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

    // Wait for all providers to finish (they exit when shutdown is cancelled).
    for handle in handles {
        let _ = handle.await;
    }

    // Wait for the router to drain its in-flight messages.
    let _ = router_handle.await;

    info!("rustifymyclaw shutdown complete");
    Ok(())
}
