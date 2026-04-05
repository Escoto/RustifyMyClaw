use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::types::{AllowedUser, ChatId, FormattedResponse, InboundMessage};

pub mod telegram;

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
    /// This method runs indefinitely (polling loop). Spawn it as a Tokio task.
    async fn start(
        &self,
        tx: mpsc::Sender<InboundMessage>,
        self_arc: Arc<dyn ChannelProvider>,
    ) -> Result<()>;

    /// Send a formatted response back to the originating chat.
    async fn send_response(&self, chat_id: &ChatId, response: FormattedResponse) -> Result<()>;

    /// Resolve the `AllowedUser` list for this channel into a set of platform-native
    /// user ID strings suitable for `SecurityGate` comparison.
    async fn resolve_users(&self, users: &[AllowedUser]) -> Result<HashSet<String>>;
}
