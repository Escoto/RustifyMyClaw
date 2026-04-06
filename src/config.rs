use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;

use crate::types::AllowedUser;

const KNOWN_BACKENDS: &[&str] = &["claude-cli", "codex-cli", "gemini-cli"];
const KNOWN_CHANNELS: &[&str] = &["telegram", "whatsapp", "slack"];

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub workspaces: Vec<WorkspaceConfig>,
    pub output: OutputConfig,
    /// Optional global rate-limiting policy. Absent means no rate limiting.
    #[serde(default)]
    pub limits: Option<LimitsConfig>,
}

/// Per-user rate limiting policy.
#[derive(Debug, Clone, Deserialize)]
pub struct LimitsConfig {
    /// Maximum requests allowed per user within the sliding window.
    pub max_requests: u32,
    /// Sliding window width in seconds.
    pub window_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceConfig {
    pub name: String,
    pub directory: PathBuf,
    pub backend: String,
    pub channels: Vec<ChannelConfig>,
    /// Optional CLI process timeout in seconds. Absent means no timeout.
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
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

/// Load and validate config from the default platform path.
pub fn load() -> Result<AppConfig> {
    load_from_path(&dirs_path())
}

/// Load and validate config from an explicit path.
///
/// Used by both `load()` (startup) and the config hot-reload watcher.
pub fn load_from_path(path: &Path) -> Result<AppConfig> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read config file: {}", path.display()))?;
    let interpolated = interpolate_env_vars(&raw)?;
    let config: AppConfig = serde_yaml::from_str(&interpolated)
        .context("config.yaml is malformed or missing required fields")?;
    validate(&config)?;
    Ok(config)
}

/// Return the default config file path for the current platform.
pub fn dirs_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    #[cfg(not(target_os = "windows"))]
    let base = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());

    #[cfg(target_os = "windows")]
    return PathBuf::from(base)
        .join("RustifyMyClaw")
        .join("config.yaml");
    #[cfg(not(target_os = "windows"))]
    PathBuf::from(base)
        .join(".rustifymyclaw")
        .join("config.yaml")
}

/// Log which differences between `old` and `new` are safe to apply vs require restart.
///
/// Called by the config watcher after a successful reload to inform the operator.
pub fn diff_reload(old: &AppConfig, new: &AppConfig) {
    // Safe-to-reload fields
    if old.output.max_message_chars != new.output.max_message_chars
        || old.output.file_upload_threshold_bytes != new.output.file_upload_threshold_bytes
        || old.output.chunk_strategy != new.output.chunk_strategy
    {
        tracing::info!("config change detected: output settings (requires restart to apply)");
    }

    let old_limits = old
        .limits
        .as_ref()
        .map(|l| (l.max_requests, l.window_seconds));
    let new_limits = new
        .limits
        .as_ref()
        .map(|l| (l.max_requests, l.window_seconds));
    if old_limits != new_limits {
        tracing::info!("config change detected: rate limits (hot-reloaded)");
    }

    // Restart-required changes
    let old_ws_names: std::collections::HashSet<&str> =
        old.workspaces.iter().map(|w| w.name.as_str()).collect();
    let new_ws_names: std::collections::HashSet<&str> =
        new.workspaces.iter().map(|w| w.name.as_str()).collect();
    if old_ws_names != new_ws_names {
        tracing::warn!(
            "config change detected: workspaces added or removed — restart required to apply"
        );
    }

    for old_ws in &old.workspaces {
        if let Some(new_ws) = new.workspaces.iter().find(|w| w.name == old_ws.name) {
            if old_ws.backend != new_ws.backend {
                tracing::warn!(
                    workspace = old_ws.name,
                    "config change detected: backend changed — restart required to apply"
                );
            }
            if old_ws.timeout_seconds != new_ws.timeout_seconds {
                tracing::info!(
                    workspace = old_ws.name,
                    "config change detected: timeout_seconds (requires restart to apply)"
                );
            }
            let old_ch_tokens: Vec<&str> =
                old_ws.channels.iter().map(|c| c.token.as_str()).collect();
            let new_ch_tokens: Vec<&str> =
                new_ws.channels.iter().map(|c| c.token.as_str()).collect();
            if old_ch_tokens != new_ch_tokens {
                tracing::warn!(
                    workspace = old_ws.name,
                    "config change detected: channel tokens changed — restart required to apply"
                );
            }
            let old_users: Vec<_> = old_ws
                .channels
                .iter()
                .flat_map(|c| &c.allowed_users)
                .collect();
            let new_users: Vec<_> = new_ws
                .channels
                .iter()
                .flat_map(|c| &c.allowed_users)
                .collect();
            if old_users.len() != new_users.len() {
                tracing::warn!(
                    workspace = old_ws.name,
                    "config change detected: allowed_users changed — restart required to apply"
                );
            }
        }
    }
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
