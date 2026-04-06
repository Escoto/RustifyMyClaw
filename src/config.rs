use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;

use crate::types::AllowedUser;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub workspaces: Vec<WorkspaceConfig>,
    pub output: OutputConfig,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceConfig {
    pub name: String,
    pub directory: PathBuf,
    pub backend: String,
    pub channels: Vec<ChannelConfig>,
}

#[derive(Debug, Deserialize)]
pub struct ChannelConfig {
    pub kind: String,
    pub bot_name: Option<String>,
    pub token: String,
    pub allowed_users: Vec<AllowedUser>,
    // Per-channel output overrides — each falls back to the global OutputConfig when absent.
    #[serde(default)]
    pub max_message_chars: Option<usize>,
    #[serde(default)]
    pub file_upload_threshold_bytes: Option<usize>,
    // WhatsApp Business Cloud API fields.
    #[serde(default)]
    pub webhook_port: Option<u16>,
    #[serde(default)]
    pub phone_number_id: Option<String>,
    #[serde(default)]
    pub verify_token: Option<String>,
    // Slack-specific fields.
    /// xapp-* Socket Mode token (required for Slack channels).
    #[serde(default)]
    pub app_token: Option<String>,
    /// When true, responses are sent as thread replies in Slack.
    #[serde(default)]
    pub use_threads: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OutputConfig {
    pub max_message_chars: usize,
    pub file_upload_threshold_bytes: usize,
    pub chunk_strategy: ChunkStrategy,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum ChunkStrategy {
    #[serde(rename = "natural")]
    Natural,
    #[serde(rename = "fixed")]
    Fixed,
}

/// Produce an effective `OutputConfig` by merging per-channel overrides onto the global defaults.
///
/// Any field present in `channel` takes precedence over the corresponding field in `global`.
/// The `chunk_strategy` is always inherited from the global config (no per-channel override).
pub fn effective_output_config(global: &OutputConfig, channel: &ChannelConfig) -> OutputConfig {
    OutputConfig {
        max_message_chars: channel
            .max_message_chars
            .unwrap_or(global.max_message_chars),
        file_upload_threshold_bytes: channel
            .file_upload_threshold_bytes
            .unwrap_or(global.file_upload_threshold_bytes),
        chunk_strategy: global.chunk_strategy.clone(),
    }
}

const KNOWN_BACKENDS: &[&str] = &["claude-cli", "codex-cli", "gemini-cli"];
const KNOWN_CHANNELS: &[&str] = &["telegram", "whatsapp", "slack"];

/// Load and validate config from `~/.rustifymyclaw/config.yaml`.
pub fn load() -> Result<AppConfig> {
    let path = dirs_path();
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read config file: {}", path.display()))?;
    let interpolated = interpolate_env_vars(&raw)?;
    let config: AppConfig = serde_yaml::from_str(&interpolated)
        .context("config.yaml is malformed or missing required fields")?;
    validate(&config)?;
    Ok(config)
}

fn dirs_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".rustifymyclaw")
        .join("config.yaml")
}

/// Replace all `${VAR_NAME}` occurrences with the corresponding environment variable.
/// Returns an error if any referenced variable is not set.
fn interpolate_env_vars(raw: &str) -> Result<String> {
    let re = regex::Regex::new(r"\$\{([^}]+)\}").expect("static regex is valid");
    let mut result = String::with_capacity(raw.len());
    let mut last_end = 0;

    for cap in re.captures_iter(raw) {
        let full = cap.get(0).unwrap();
        let var_name = cap.get(1).unwrap().as_str();
        let value = std::env::var(var_name).map_err(|_| {
            anyhow!("environment variable `{var_name}` is not set (required by config)")
        })?;
        result.push_str(&raw[last_end..full.start()]);
        result.push_str(&value);
        last_end = full.end();
    }
    result.push_str(&raw[last_end..]);
    Ok(result)
}

fn validate(config: &AppConfig) -> Result<()> {
    if config.workspaces.is_empty() {
        bail!("config must define at least one workspace");
    }

    for ws in &config.workspaces {
        if ws.name.is_empty() {
            bail!("workspace has an empty name");
        }
        if !ws.directory.exists() {
            bail!(
                "workspace `{}`: directory `{}` does not exist",
                ws.name,
                ws.directory.display()
            );
        }
        if !KNOWN_BACKENDS.contains(&ws.backend.as_str()) {
            bail!(
                "workspace `{}`: unknown backend `{}` (known: {})",
                ws.name,
                ws.backend,
                KNOWN_BACKENDS.join(", ")
            );
        }
        if ws.channels.is_empty() {
            bail!("workspace `{}` must define at least one channel", ws.name);
        }
        for ch in &ws.channels {
            if !KNOWN_CHANNELS.contains(&ch.kind.as_str()) {
                bail!(
                    "workspace `{}`: unknown channel kind `{}` (known: {})",
                    ws.name,
                    ch.kind,
                    KNOWN_CHANNELS.join(", ")
                );
            }
            if ch.allowed_users.is_empty() {
                bail!(
                    "workspace `{}`, channel `{}`: allowed_users must be non-empty",
                    ws.name,
                    ch.kind
                );
            }
            warn_misplaced_fields(&ws.name, ch);
        }
    }
    Ok(())
}

/// Warn the operator if platform-specific fields are present on the wrong channel kind.
///
/// Fields are silently ignored at runtime (main.rs only reads them in the matching arm),
/// but a typo or copy-paste error would otherwise be swallowed with no feedback.
fn warn_misplaced_fields(ws_name: &str, ch: &ChannelConfig) {
    // WhatsApp-specific fields on a non-WhatsApp channel.
    if ch.kind != "whatsapp" {
        if ch.phone_number_id.is_some() {
            tracing::warn!(
                workspace = ws_name,
                kind = ch.kind,
                "`phone_number_id` is a WhatsApp-only field and will be ignored"
            );
        }
        if ch.webhook_port.is_some() {
            tracing::warn!(
                workspace = ws_name,
                kind = ch.kind,
                "`webhook_port` is a WhatsApp-only field and will be ignored"
            );
        }
        if ch.verify_token.is_some() {
            tracing::warn!(
                workspace = ws_name,
                kind = ch.kind,
                "`verify_token` is a WhatsApp-only field and will be ignored"
            );
        }
    }
    // Slack-specific fields on a non-Slack channel.
    if ch.kind != "slack" {
        if ch.app_token.is_some() {
            tracing::warn!(
                workspace = ws_name,
                kind = ch.kind,
                "`app_token` is a Slack-only field and will be ignored"
            );
        }
        if ch.use_threads.is_some() {
            tracing::warn!(
                workspace = ws_name,
                kind = ch.kind,
                "`use_threads` is a Slack-only field and will be ignored"
            );
        }
    }
}

#[cfg(test)]
#[path = "tests/config_test.rs"]
mod tests;
