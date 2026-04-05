use std::path::Path;

use anyhow::{bail, Result};
use async_trait::async_trait;
use tokio::process::Command;

use crate::types::{CliResponse, SessionState};

pub mod claude;
pub mod codex;
pub mod gemini;

/// Abstraction over a CLI AI backend.
///
/// Each backend knows its binary name, how to build a command for a given prompt,
/// and how to interpret the raw output. The executor calls these methods — it
/// never knows which concrete backend it's running.
#[async_trait]
pub trait CliBackend: Send + Sync {
    /// Construct the `tokio::process::Command` to spawn for this prompt.
    fn build_command(&self, prompt: &str, working_dir: &Path, session: &SessionState) -> Command;

    /// Parse raw stdout into a `CliResponse`. For Phase 1 this is a pass-through;
    /// future backends may extract structured data.
    fn parse_output(
        &self,
        stdout: String,
        stderr: String,
        exit_code: i32,
        duration: std::time::Duration,
    ) -> CliResponse;

    /// Human-readable backend identifier (e.g. `"claude-cli"`).
    fn name(&self) -> &'static str;
}

/// Resolve a backend name from config to a concrete `CliBackend` implementation.
pub fn build(backend_name: &str) -> Result<Box<dyn CliBackend>> {
    match backend_name {
        "claude-cli" => Ok(Box::new(claude::ClaudeCodeBackend)),
        "codex-cli" => Ok(Box::new(codex::CodexBackend)),
        "gemini-cli" => Ok(Box::new(gemini::GeminiBackend)),
        other => bail!("unknown backend: `{other}`"),
    }
}

#[cfg(test)]
#[path = "../tests/backend/registry_test.rs"]
mod registry_tests;
