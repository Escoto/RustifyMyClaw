use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;

mod backend;
mod channel;
mod cli;
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
    let args = cli::Cli::parse();

    let log_filter = args
        .log_level
        .as_deref()
        .map(tracing_subscriber::EnvFilter::new)
        .or_else(|| tracing_subscriber::EnvFilter::try_from_default_env().ok())
        .unwrap_or_else(|| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt().with_env_filter(log_filter).init();

    if cli::run_command(&args)? {
        return Ok(());
    }

    let config_path = config::resolve_path(args.config_file);
    tracing::info!(path = %config_path.display(), "resolved config path");
    let app_config =
        config::load_from_path(&config_path).context("failed to load configuration")?;

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

    spawn_config_watcher(config_path, app_config, shutdown, rate_limiter_shared);

    await_shutdown(provider_handles, router_handle).await;
    Ok(())
}
