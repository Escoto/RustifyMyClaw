use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use teloxide::prelude::*;
use teloxide::types::InputFile;
use tokio::sync::mpsc;

use tokio::sync::RwLock;

use crate::channel::ChannelProvider;
use crate::security::SecurityGate;
use crate::types::{
    AllowedUser, ChannelKind, ChatId, FormattedResponse, InboundMessage, MessageContext,
    ResponseChunk, WorkspaceHandle,
};

const TELEGRAM_MAX_CHARS: usize = 4096;
const TRUNCATION_SUFFIX: &str = "\n[truncated]";

/// Telegram channel provider using teloxide in polling mode.
pub struct TelegramProvider {
    bot: Bot,
    security_gate: SecurityGate,
    workspace: Arc<RwLock<WorkspaceHandle>>,
}

impl TelegramProvider {
    pub fn new(
        token: String,
        security_gate: SecurityGate,
        workspace: Arc<RwLock<WorkspaceHandle>>,
    ) -> Self {
        Self {
            bot: Bot::new(token),
            security_gate,
            workspace,
        }
    }
}

#[async_trait]
impl ChannelProvider for TelegramProvider {
    async fn start(
        &self,
        tx: mpsc::Sender<InboundMessage>,
        self_arc: Arc<dyn ChannelProvider>,
    ) -> Result<()> {
        let bot = self.bot.clone();
        let gate = self.security_gate.clone();
        let workspace = Arc::clone(&self.workspace);

        teloxide::repl(bot, move |_bot: Bot, msg: Message| {
            let tx = tx.clone();
            let gate = gate.clone();
            let workspace = Arc::clone(&workspace);
            let provider = Arc::clone(&self_arc);

            async move {
                let Some(text) = msg.text() else {
                    return Ok(());
                };

                let user_id = match &msg.from {
                    Some(u) => u.id.0.to_string(),
                    None => {
                        tracing::trace!("telegram message with no sender — dropped");
                        return Ok(());
                    }
                };

                if !gate.is_allowed(&user_id) {
                    tracing::trace!(user_id, "unauthorized telegram message — dropped");
                    return Ok(());
                }

                let chat_id = ChatId {
                    channel: ChannelKind::Telegram,
                    platform_id: msg.chat.id.0.to_string(),
                };

                let inbound = InboundMessage {
                    chat_id,
                    user_id,
                    text: text.to_string(),
                    timestamp: Utc::now(),
                    context: MessageContext {
                        workspace: Arc::clone(&workspace),
                        provider,
                    },
                };

                if tx.send(inbound).await.is_err() {
                    tracing::error!("router channel closed — cannot forward telegram message");
                }

                Ok(())
            }
        })
        .await;

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

    async fn resolve_users(&self, users: &[AllowedUser]) -> Result<HashSet<String>> {
        let mut resolved = HashSet::new();
        for user in users {
            match user {
                AllowedUser::NumericId(id) => {
                    resolved.insert(id.to_string());
                }
                AllowedUser::Handle(handle) => {
                    // The Telegram Bot API does not expose a username→ID lookup endpoint.
                    // We store the handle as-is and warn the operator. Numeric IDs are
                    // preferred for reliable allow-listing.
                    tracing::warn!(
                        handle,
                        "Telegram username resolution via Bot API is not supported without \
                         prior interaction. Storing handle as-is; numeric IDs are more reliable."
                    );
                    resolved.insert(handle.clone());
                }
            }
        }
        Ok(resolved)
    }
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
