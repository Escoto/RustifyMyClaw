use std::sync::Arc;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::config::{ChannelConfig, OutputConfig};
use crate::types::{ChatId, FormattedResponse, InboundMessage, WorkspaceHandle};

pub mod slack;
pub mod telegram;
pub mod whatsapp;

/// Abstraction over a messaging platform.
///
/// Each platform implements this trait to normalize its API behind a common interface.
/// The executor and router never touch platform-specific types.
#[async_trait]
pub trait ChannelProvider: Send + Sync {
    /// Start receiving messages and forward them to `tx`.
    ///
    /// `self_arc` is the same `Arc` that will be embedded in each `MessageContext` so
    /// the router can call `send_response` on the originating provider. Passing it here
    /// avoids self-referential struct construction in the provider.
    ///
    /// `shutdown` is cancelled when the daemon is stopping. The provider must exit
    /// promptly when `shutdown.cancelled()` resolves.
    ///
    /// This method runs indefinitely (polling loop). Spawn it as a Tokio task.
    async fn start(
        &self,
        tx: mpsc::Sender<InboundMessage>,
        self_arc: Arc<dyn ChannelProvider>,
        shutdown: CancellationToken,
    ) -> Result<()>;

    /// Send a formatted response back to the originating chat.
    async fn send_response(&self, chat_id: &ChatId, response: FormattedResponse) -> Result<()>;
}

/// Factory trait for constructing a [`ChannelProvider`] from configuration.
///
/// Each provider implements this to encapsulate its own config-field validation,
/// user resolution, and [`SecurityGate`] construction. The companion [`build`]
/// function dispatches to the correct implementation by channel kind.
#[async_trait]
pub trait ChannelProviderFactory: ChannelProvider + Sized {
    /// Build a fully-initialised provider from a channel config block.
    ///
    /// Implementations should:
    /// 1. Validate provider-specific fields in `ch_config`.
    /// 2. Resolve allowed users via the module-level `resolve_users` function.
    /// 3. Build the provider with the resolved gate and effective output config.
    async fn create(
        ch_config: &ChannelConfig,
        workspace: Arc<RwLock<WorkspaceHandle>>,
        global_output: &Arc<OutputConfig>,
    ) -> Result<Arc<dyn ChannelProvider>>;
}

/// Construct a [`ChannelProvider`] for the given channel config block.
///
/// This is the single entry point that `main.rs` calls for every configured channel.
pub async fn build(
    ch_config: &ChannelConfig,
    workspace_name: &str,
    workspace: Arc<RwLock<WorkspaceHandle>>,
    global_output: &Arc<OutputConfig>,
) -> Result<Arc<dyn ChannelProvider>> {
    let provider: Arc<dyn ChannelProvider> = match ch_config.kind.as_str() {
        "telegram" => telegram::TelegramProvider::create(ch_config, workspace, global_output).await,
        "whatsapp" => whatsapp::WhatsAppProvider::create(ch_config, workspace, global_output).await,
        "slack" => slack::SlackProvider::create(ch_config, workspace, global_output).await,
        other => bail!("channel kind `{other}` is not implemented"),
    }
    .with_context(|| {
        format!(
            "workspace `{workspace_name}`: failed to build `{}` channel",
            ch_config.kind
        )
    })?;

    info!(
        workspace = workspace_name,
        kind = ch_config.kind,
        bot_name = ch_config.bot_name.as_deref().unwrap_or("(unnamed)"),
        "channel registered"
    );

    Ok(provider)
}
