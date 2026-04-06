use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::backend::CliBackend;
use crate::command::BridgeCommand;
use crate::executor::Executor;
use crate::formatter;
use crate::rate_limit::{RateLimitResult, RateLimiter};
use crate::session::SessionStore;
use crate::types::{FormattedResponse, InboundMessage, ResponseChunk, WorkspaceHandle};

/// How long to wait for in-flight messages to complete after shutdown is signalled.
const DRAIN_TIMEOUT: Duration = Duration::from_secs(30);

/// Orchestrates the message pipeline: parse → session → execute → format → respond.
///
/// Each router is wired to a fixed set of backends (keyed by backend name), a shared
/// session store, and the full pool of available workspaces for `/use` switching.
/// Output formatting uses the per-channel `OutputConfig` carried in each `MessageContext`.
pub struct Router {
    session_store: Arc<RwLock<SessionStore>>,
    /// Backend registry: backend name → implementation.
    backends: HashMap<String, Arc<dyn CliBackend>>,
    /// All configured workspaces, keyed by name — used by `/use` to validate and swap.
    available_workspaces: HashMap<String, WorkspaceHandle>,
    /// Optional per-user rate limiter. `None` means no rate limiting.
    rate_limiter: Option<Arc<RateLimiter>>,
}

impl Router {
    pub fn new(
        session_store: Arc<RwLock<SessionStore>>,
        backends: HashMap<String, Arc<dyn CliBackend>>,
        available_workspaces: HashMap<String, WorkspaceHandle>,
        rate_limiter: Option<Arc<RateLimiter>>,
    ) -> Self {
        Self {
            session_store,
            backends,
            available_workspaces,
            rate_limiter,
        }
    }

    /// Run the router loop until the channel closes or `shutdown` is cancelled.
    ///
    /// On shutdown, stops accepting new messages and drains in-flight handlers
    /// (up to `DRAIN_TIMEOUT`).
    pub async fn run(
        self: Arc<Self>,
        mut rx: mpsc::Receiver<InboundMessage>,
        shutdown: CancellationToken,
    ) {
        let mut join_set: JoinSet<()> = JoinSet::new();

        loop {
            tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        Some(msg) => {
                            let router = Arc::clone(&self);
                            join_set.spawn(async move {
                                if let Err(e) = router.handle(msg).await {
                                    error!(error = ?e, "error handling message");
                                }
                            });
                        }
                        None => {
                            // All senders dropped — channel closed.
                            info!("router channel closed — exiting");
                            break;
                        }
                    }
                }
                _ = shutdown.cancelled() => {
                    info!("router received shutdown signal — draining in-flight messages");
                    break;
                }
                // Reap completed tasks without blocking.
                Some(_) = join_set.join_next(), if !join_set.is_empty() => {}
            }
        }

        // Drain any remaining in-flight handlers.
        if !join_set.is_empty() {
            let count = join_set.len();
            info!(count, "waiting for in-flight messages to complete");
            match tokio::time::timeout(DRAIN_TIMEOUT, async {
                while join_set.join_next().await.is_some() {}
            })
            .await
            {
                Ok(_) => info!("all in-flight messages completed"),
                Err(_) => {
                    warn!("drain timeout exceeded — aborting remaining tasks");
                    join_set.abort_all();
                }
            }
        }

        info!("router exiting");
    }

    async fn handle(&self, msg: InboundMessage) -> Result<()> {
        let command = BridgeCommand::parse(&msg.text);
        let chat_id = msg.chat_id.clone();
        let workspace_arc = Arc::clone(&msg.context.workspace);
        let provider = Arc::clone(&msg.context.provider);
        let output_config = Arc::clone(&msg.context.output_config);

        match command {
            BridgeCommand::NewSession => {
                self.session_store.write().await.reset(&chat_id);
                let response = FormattedResponse {
                    chunks: vec![ResponseChunk::Text(
                        "Session reset. Next message will start a new conversation.".to_string(),
                    )],
                };
                provider.send_response(&chat_id, response).await?;
            }

            BridgeCommand::Status => {
                let session = self.session_store.read().await.get(&chat_id);
                let (ws_name, ws_backend) = {
                    let ws = workspace_arc.read().await;
                    (ws.name.clone(), ws.backend.clone())
                };
                let status = format!(
                    "Workspace: {}\nBackend: {}\nSession: {}",
                    ws_name,
                    ws_backend,
                    if session.is_active { "active" } else { "new" }
                );
                let response = FormattedResponse {
                    chunks: vec![ResponseChunk::Text(status)],
                };
                provider.send_response(&chat_id, response).await?;
            }

            BridgeCommand::Help => {
                let help = "\
/new         — start a fresh session (clears conversation history)\n\
/status      — show workspace, backend, and session info\n\
/use <name>  — switch to a different workspace\n\
/help        — show this message\n\
Any other message is forwarded to the AI backend.";
                let response = FormattedResponse {
                    chunks: vec![ResponseChunk::Text(help.to_string())],
                };
                provider.send_response(&chat_id, response).await?;
            }

            BridgeCommand::UseWorkspace { name } => {
                match self.available_workspaces.get(&name) {
                    Some(new_ws) => {
                        // Swap workspace inside the Arc<RwLock<>> so all future messages
                        // from this channel see the new workspace immediately.
                        let (ws_name, ws_dir, ws_backend) = {
                            let mut ws = workspace_arc.write().await;
                            *ws = new_ws.clone();
                            (ws.name.clone(), ws.directory.clone(), ws.backend.clone())
                        };
                        // Session from the old workspace is meaningless in the new one.
                        self.session_store.write().await.reset(&chat_id);
                        let confirmation = format!(
                            "Switched to workspace `{}`.\nDirectory: {}\nBackend: {}",
                            ws_name,
                            ws_dir.display(),
                            ws_backend,
                        );
                        let response = FormattedResponse {
                            chunks: vec![ResponseChunk::Text(confirmation)],
                        };
                        provider.send_response(&chat_id, response).await?;
                    }
                    None => {
                        let available: Vec<&str> = self
                            .available_workspaces
                            .keys()
                            .map(String::as_str)
                            .collect();
                        let mut names = available.clone();
                        names.sort_unstable();
                        let error_msg = format!(
                            "Unknown workspace `{name}`. Available: {}",
                            names.join(", ")
                        );
                        let response = FormattedResponse {
                            chunks: vec![ResponseChunk::Text(error_msg)],
                        };
                        provider.send_response(&chat_id, response).await?;
                    }
                }
            }

            BridgeCommand::Prompt { text } => {
                // Check rate limit before doing any expensive work.
                if let Some(limiter) = &self.rate_limiter {
                    if let RateLimitResult::LimitedFor(wait) = limiter.check(&msg.user_id) {
                        let secs = wait.as_secs().max(1);
                        let response = FormattedResponse {
                            chunks: vec![ResponseChunk::Text(format!(
                                "Rate limit exceeded. Try again in {secs}s."
                            ))],
                        };
                        provider.send_response(&chat_id, response).await?;
                        return Ok(());
                    }
                }

                let (ws_name, ws_dir, ws_backend, ws_timeout) = {
                    let ws = workspace_arc.read().await;
                    (
                        ws.name.clone(),
                        ws.directory.clone(),
                        ws.backend.clone(),
                        ws.timeout,
                    )
                };
                let _ = ws_name; // available for future logging

                let backend = match self.backends.get(&ws_backend) {
                    Some(b) => Arc::clone(b),
                    None => {
                        error!(
                            backend = ws_backend,
                            "no backend registered for workspace — cannot handle prompt"
                        );
                        return Ok(());
                    }
                };

                let session = self.session_store.read().await.get(&chat_id);
                let workspace_handle = WorkspaceHandle {
                    name: ws_name,
                    directory: ws_dir,
                    backend: ws_backend,
                    timeout: ws_timeout,
                };
                let executor = Executor::new(backend);
                let cli_result = executor.run(&text, &workspace_handle, &session).await;

                // Mark session active regardless of outcome — the conversation started.
                self.session_store.write().await.mark_active(&chat_id);

                let output = match cli_result {
                    Err(e) => {
                        let msg_str = e.to_string();
                        if msg_str.contains("timed out") {
                            msg_str
                        } else {
                            return Err(e);
                        }
                    }
                    Ok(cli_resp) => {
                        if cli_resp.exit_code != 0 {
                            let error_prefix =
                                formatter::format_error(cli_resp.exit_code, &cli_resp.stderr);
                            if cli_resp.stdout.is_empty() {
                                error_prefix
                            } else {
                                format!("{}\n\n{}", error_prefix, cli_resp.stdout)
                            }
                        } else {
                            cli_resp.stdout
                        }
                    }
                };

                let response = formatter::format(&output, &output_config);
                provider.send_response(&chat_id, response).await?;
            }
        }

        Ok(())
    }
}
