use std::path::Path;
use std::time::Duration;

use tokio::process::Command;

use crate::types::{CliResponse, SessionState};

use super::CliBackend;

/// Backend for the Claude Code CLI (`claude`).
///
/// Invocation shape:
///   `claude -p "<prompt>"` — fresh session
///   `claude -p "<prompt>" -c` — continue existing session
pub struct ClaudeCodeBackend;

impl CliBackend for ClaudeCodeBackend {
    fn build_command(&self, prompt: &str, working_dir: &Path, session: &SessionState) -> Command {
        let mut cmd = Command::new("claude");
        cmd.arg("-p").arg(prompt);
        if session.is_active {
            cmd.arg("-c");
        }
        cmd.current_dir(working_dir);
        // Prevent the subprocess from inheriting a terminal that could cause interactive prompts.
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
        "claude-cli"
    }
}

#[cfg(test)]
#[path = "../tests/backend/claude_test.rs"]
mod tests;
