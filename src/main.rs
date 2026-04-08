use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;

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
mod startup;
mod types;

use crate::session::SessionStore;
use crate::startup::{
    await_shutdown, build_rate_limiter, build_workspaces, spawn_config_watcher, spawn_providers,
    spawn_router, spawn_signal_handler, WorkspaceSetup,
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let app_config = config::load().context("failed to load configuration")?;

    let shutdown = CancellationToken::new();
    spawn_signal_handler(shutdown.clone());

    let global_output = Arc::new(app_config.output.clone());
    let (rate_limiter, rate_limiter_shared) = build_rate_limiter(&app_config);
    let session_store = Arc::new(RwLock::new(SessionStore::new()));
    let (tx, rx) = mpsc::channel(64);

    let WorkspaceSetup {
        backends,
        workspaces,
        providers,
    } = build_workspaces(&app_config, &global_output).await?;

    let router_handle = spawn_router(
        session_store,
        backends,
        workspaces,
        rate_limiter,
        rx,
        shutdown.clone(),
    );
    let provider_handles = spawn_providers(providers, tx, shutdown.clone());

    spawn_config_watcher(app_config, shutdown, rate_limiter_shared);

    await_shutdown(provider_handles, router_handle).await;
    Ok(())
}
