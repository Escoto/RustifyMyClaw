use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use tokio::sync::{mpsc, RwLock};
use tracing::info;

mod backend;
mod channel;
mod command;
mod config;
mod executor;
mod formatter;
mod router;
mod security;
mod session;
mod types;

use crate::channel::telegram::TelegramProvider;
use crate::channel::ChannelProvider;
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

    let output_config = Arc::new(app_config.output);

    // One session store shared across all workspaces.
    let session_store = Arc::new(RwLock::new(SessionStore::new()));

    // One inbound message channel — all providers funnel into the single router.
    let (tx, rx) = mpsc::channel(64);

    // Collect all backend implementations keyed by name for the router registry.
    let mut backends: HashMap<String, Arc<dyn backend::CliBackend>> = HashMap::new();

    // All workspace handles keyed by name — used by `/use` for runtime switching.
    let mut available_workspaces: HashMap<String, WorkspaceHandle> = HashMap::new();

    let mut provider_arcs: Vec<Arc<dyn ChannelProvider>> = Vec::new();

    for ws_config in app_config.workspaces {
        let workspace_handle = WorkspaceHandle {
            name: ws_config.name.clone(),
            directory: ws_config.directory.clone(),
            backend: ws_config.backend.clone(),
        };

        available_workspaces.insert(ws_config.name.clone(), workspace_handle.clone());

        // Wrap in Arc<RwLock<>> so the /use command can swap the workspace at runtime.
        let workspace = Arc::new(RwLock::new(workspace_handle));

        // Register backend if not already present.
        if !backends.contains_key(&ws_config.backend) {
            let b = backend::build(&ws_config.backend)?;
            backends.insert(ws_config.backend.clone(), Arc::from(b));
        }

        for ch_config in ws_config.channels {
            match ch_config.kind.as_str() {
                "telegram" => {
                    // Build a temporary provider just to call resolve_users (no clone cost).
                    let tmp = TelegramProvider::new(
                        ch_config.token.clone(),
                        SecurityGate::new(Default::default()),
                        Arc::clone(&workspace),
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
                    let provider: Arc<dyn ChannelProvider> = Arc::new(TelegramProvider::new(
                        ch_config.token,
                        gate,
                        Arc::clone(&workspace),
                    ));

                    info!(
                        workspace = ws_config.name,
                        bot_name = ch_config.bot_name.as_deref().unwrap_or("(unnamed)"),
                        "telegram channel registered"
                    );

                    provider_arcs.push(provider);
                }
                other => {
                    bail!("channel kind `{other}` is not implemented");
                }
            }
        }
    }

    // Start the router.
    let router = Arc::new(Router::new(
        Arc::clone(&session_store),
        output_config,
        backends,
        available_workspaces,
    ));
    tokio::spawn({
        let router = Arc::clone(&router);
        async move { router.run(rx).await }
    });

    // Start all channel providers.
    let mut handles = Vec::new();
    for provider in provider_arcs {
        let tx_clone = tx.clone();
        let provider_clone = Arc::clone(&provider);
        handles.push(tokio::spawn(async move {
            if let Err(e) = provider_clone.start(tx_clone, provider).await {
                tracing::error!(error = ?e, "channel provider crashed");
            }
        }));
    }

    // Drop the original sender so the router exits when all providers do.
    drop(tx);

    for handle in handles {
        let _ = handle.await;
    }

    Ok(())
}
