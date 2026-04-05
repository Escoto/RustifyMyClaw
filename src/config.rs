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
}

#[derive(Debug, Deserialize)]
pub struct OutputConfig {
    pub max_message_chars: usize,
    pub file_upload_threshold_bytes: usize,
    pub chunk_strategy: ChunkStrategy,
}

#[derive(Debug, Deserialize, PartialEq)]
pub enum ChunkStrategy {
    #[serde(rename = "natural")]
    Natural,
    #[serde(rename = "fixed")]
    Fixed,
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
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/config_test.rs"]
mod tests;
