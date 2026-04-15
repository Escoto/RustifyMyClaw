use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use teloxide::prelude::*;
use teloxide::types::InputFile;
use tokio::sync::mpsc;

use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::channel::{ChannelProvider, ChannelProviderFactory};
use crate::config::{self, ChannelConfig, OutputConfig};
use crate::security::SecurityGate;
use crate::types::{
    AllowedUser, ChatId, FormattedResponse, InboundMessage, ResponseChunk, WorkspaceHandle,
};

const TELEGRAM_MAX_CHARS: usize = 4096;
const TRUNCATION_SUFFIX: &str = "\n[truncated]";

/// Telegram channel provider using teloxide in polling mode.
pub struct TelegramProvider {
    bot: Bot,
    security_gate: SecurityGate,
    workspace: Arc<RwLock<WorkspaceHandle>>,
    output_config: Arc<OutputConfig>,
}

impl TelegramProvider {
    pub fn new(
        token: String,
        security_gate: SecurityGate,
        workspace: Arc<RwLock<WorkspaceHandle>>,
        output_config: Arc<OutputConfig>,
    ) -> Self {
        Self {
            bot: Bot::new(token),
            security_gate,
            workspace,
            output_config,
        }
    }
}

#[async_trait]
impl ChannelProviderFactory for TelegramProvider {
    async fn create(
        ch_config: &ChannelConfig,
        workspace: Arc<RwLock<WorkspaceHandle>>,
        global_output: &Arc<OutputConfig>,
    ) -> Result<Arc<dyn ChannelProvider>> {
        let resolved = resolve_users(&ch_config.allowed_users)?;
        let gate = SecurityGate::new(resolved);
        let effective_output = Arc::new(config::effective_output_config(global_output, ch_config));
        Ok(Arc::new(Self::new(
            ch_config.token.clone(),
            gate,
            workspace,
            effective_output,
        )))
    }
}

#[async_trait]
impl ChannelProvider for TelegramProvider {
    async fn start(
        &self,
        tx: mpsc::Sender<InboundMessage>,
        self_arc: Arc<dyn ChannelProvider>,
        shutdown: CancellationToken,
    ) -> Result<()> {
        let bot = self.bot.clone();
        let gate = self.security_gate.clone();
        let workspace = Arc::clone(&self.workspace);
        let output_config = Arc::clone(&self.output_config);

        let repl_future = teloxide::repl(bot, move |_bot: Bot, msg: Message| {
            let tx = tx.clone();
            let gate = gate.clone();
            let workspace = Arc::clone(&workspace);
            let output_config = Arc::clone(&output_config);
            let provider = Arc::clone(&self_arc);

            async move {
                let Some(text) = msg.text() else {
                    return Ok(());
                };

                let user = match &msg.from {
                    Some(u) => u,
                    None => {
                        tracing::trace!("telegram message with no sender — dropped");
                        return Ok(());
                    }
                };
                let user_id = user.id.0.to_string();

                let username_allowed = user
                    .username
                    .as_deref()
                    .is_some_and(|name| gate.is_allowed(&name.to_lowercase()));

                if !username_allowed && !gate.is_allowed(&user_id) {
                    tracing::trace!(user_id, "unauthorized telegram message — dropped");
                    return Ok(());
                }

                let chat_id = ChatId::telegram(&msg.chat.id.0.to_string());

                let inbound = InboundMessage::new(
                    chat_id,
                    user_id,
                    text.to_string(),
                    &workspace,
                    &provider,
                    &output_config,
                );

                if tx.send(inbound).await.is_err() {
                    tracing::error!("router channel closed — cannot forward telegram message");
                }

                Ok(())
            }
        });

        tokio::select! {
            _ = repl_future => {}
            _ = shutdown.cancelled() => {
                tracing::info!("telegram polling loop shutting down");
            }
        }

        Ok(())
    }

    async fn send_response(&self, chat_id: &ChatId, response: FormattedResponse) -> Result<()> {
        let tg_chat_id = chat_id
            .platform_id
            .parse::<i64>()
            .context("invalid telegram chat_id")?;
        let tg_chat = teloxide::types::ChatId(tg_chat_id);

        for chunk in response.chunks {
            match chunk {
                ResponseChunk::Text(text) => {
                    let safe = enforce_telegram_limit(&text);
                    self.bot
                        .send_message(tg_chat, safe)
                        .await
                        .context("failed to send telegram message")?;
                }
                ResponseChunk::File { name, content } => {
                    self.bot
                        .send_document(tg_chat, InputFile::memory(content).file_name(name))
                        .await
                        .context("failed to upload file to telegram")?;
                }
            }
        }
        Ok(())
    }
}

/// Resolve Telegram [`AllowedUser`] entries into platform-native ID strings
/// suitable for [`SecurityGate`] comparison.
///
/// Handles are stripped of a leading `@` and lowercased to match the format
/// that Telegram delivers on incoming messages.
pub fn resolve_users(users: &[AllowedUser]) -> Result<HashSet<String>> {
    let mut resolved = HashSet::new();
    for user in users {
        match user {
            AllowedUser::NumericId(id) => {
                resolved.insert(id.to_string());
            }
            AllowedUser::Handle(handle) => {
                let normalized = handle.strip_prefix('@').unwrap_or(handle).to_lowercase();
                resolved.insert(normalized);
            }
        }
    }
    Ok(resolved)
}

/// Truncate a message that exceeds Telegram's 4096-char hard limit.
fn enforce_telegram_limit(text: &str) -> String {
    if text.len() <= TELEGRAM_MAX_CHARS {
        return text.to_string();
    }
    let cut = TELEGRAM_MAX_CHARS - TRUNCATION_SUFFIX.len();
    // Walk back to a valid UTF-8 char boundary
    let mut boundary = cut;
    while boundary > 0 && !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    format!("{}{}", &text[..boundary], TRUNCATION_SUFFIX)
}

#[cfg(test)]
#[path = "../tests/channel/telegram_test.rs"]
mod tests;
