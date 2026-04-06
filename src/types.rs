use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::channel::ChannelProvider;
use crate::config::OutputConfig;

/// Platform discriminant — prevents chat ID collisions across messaging platforms.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum ChannelKind {
    Telegram,
    WhatsApp,
    Slack,
}

/// Platform-agnostic conversation identifier.
///
/// Combines channel kind + platform-native ID so that e.g. Telegram chat `12345`
/// and WhatsApp chat `12345` are distinct sessions.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ChatId {
    pub channel: ChannelKind,
    /// Normalized to `String` to accommodate all platforms:
    /// Telegram i64, WhatsApp phone number, Slack alphanumeric channel ID.
    pub platform_id: String,
}

/// Represents an allowed user in the config. Each platform has its own identity format.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum AllowedUser {
    /// Telegram numeric user ID: `987654321`
    NumericId(i64),
    /// Telegram username (`@user-x`), Slack handle, or WhatsApp phone (`+5511999999999`)
    Handle(String),
}

/// An inbound message from any messaging platform, normalized to a common shape.
pub struct InboundMessage {
    pub chat_id: ChatId,
    /// Platform-native user identifier as a string (for `SecurityGate` comparison).
    #[allow(dead_code)] // set at ingestion; available for logging/audit in later phases
    pub user_id: String,
    pub text: String,
    #[allow(dead_code)] // set at ingestion; available for rate-limiting in later phases
    pub timestamp: DateTime<Utc>,
    /// Routing context stamped by the channel listener at ingestion time.
    pub context: MessageContext,
}

/// Routing context attached by the channel listener. Carries everything the router
/// needs to execute and respond — no lookup tables required.
///
/// `workspace` is wrapped in `Arc<RwLock<>>` so the `/use` command can swap the
/// active workspace at runtime without replacing the Arc itself.
/// `output_config` is the effective per-channel config (channel overrides merged with
/// global defaults at startup), so the router always uses the correct limits.
pub struct MessageContext {
    pub workspace: Arc<RwLock<WorkspaceHandle>>,
    pub provider: Arc<dyn ChannelProvider>,
    pub output_config: Arc<OutputConfig>,
}

/// Handle to a workspace. Shared via `Arc<RwLock<>>` so the `/use` command can
/// swap it at runtime. Clone is derived for use in the available_workspaces registry.
#[derive(Clone)]
pub struct WorkspaceHandle {
    pub name: String,
    pub directory: std::path::PathBuf,
    /// Backend identifier string, e.g. `"claude-cli"`.
    pub backend: String,
}

/// Output from a single CLI invocation.
pub struct CliResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    #[allow(dead_code)] // captured for Phase 4 telemetry/timeout logic
    pub duration: Duration,
}

/// Tracks whether a conversation has an active session with the CLI backend.
#[derive(Debug, Clone)]
pub struct SessionState {
    pub is_active: bool,
    /// Reserved for Phase 4 idle-timeout logic.
    pub last_activity: DateTime<Utc>,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            is_active: false,
            last_activity: Utc::now(),
        }
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}

/// A single chunk of a formatted response sent back to the user.
pub enum ResponseChunk {
    Text(String),
    File { name: String, content: Vec<u8> },
}

/// The full formatted output ready to send via a channel provider.
/// Named struct (not type alias) to leave room for future metadata fields.
pub struct FormattedResponse {
    pub chunks: Vec<ResponseChunk>,
}
