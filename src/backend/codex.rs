use std::path::Path;
use std::time::Duration;

use tokio::process::Command;

use crate::types::{CliResponse, SessionState};

use super::CliBackend;

/// Backend for the OpenAI Codex CLI (`codex`).
///
/// Invocation shape:
///   `codex -q "<prompt>"` — non-interactive quiet mode
///
/// Codex does not support session continuation — session state is ignored.
pub struct CodexBackend;

impl CliBackend for CodexBackend {
    fn build_command(&self, prompt: &str, working_dir: &Path, _session: &SessionState) -> Command {
        let mut cmd = Command::new("codex");
        // -q: quiet / non-interactive mode
        cmd.arg("-q").arg(prompt);
        cmd.current_dir(working_dir);
        cmd.stdin(std::process::Stdio::null());
        cmd
    }

    fn parse_output(
        &self,
        stdout: String,
        stderr: String,
        exit_code: i32,
        duration: Duration,
    ) -> CliResponse {
        CliResponse {
            stdout,
            stderr,
            exit_code,
            duration,
        }
    }

    fn name(&self) -> &'static str {
        "codex-cli"
    }
}

#[cfg(test)]
#[path = "../tests/backend/codex_test.rs"]
mod tests;
