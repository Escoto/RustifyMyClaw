use std::path::Path;
use std::time::Duration;

use tokio::process::Command;

use crate::types::{CliResponse, SessionState};

use super::CliBackend;

/// Backend for the Google Gemini CLI (`gemini`).
///
/// Invocation shape:
///   `gemini -p "<prompt>" -y` — non-interactive, auto-approve
///
/// Gemini does not support session continuation — session state is ignored.
pub struct GeminiBackend;

impl CliBackend for GeminiBackend {
    fn build_command(&self, prompt: &str, working_dir: &Path, _session: &SessionState) -> Command {
        let mut cmd = Command::new("gemini");
        // -p: prompt flag; -y: auto-approve (non-interactive)
        cmd.arg("-p").arg(prompt).arg("-y");
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
        "gemini-cli"
    }
}

#[cfg(test)]
#[path = "../tests/backend/gemini_test.rs"]
mod tests;
