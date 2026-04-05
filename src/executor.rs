use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::io::AsyncReadExt;

use crate::backend::CliBackend;
use crate::types::{CliResponse, SessionState, WorkspaceHandle};

/// Executes a CLI backend command and captures its output.
///
/// This is a dumb pipe — it spawns the process, waits, and returns the raw data.
/// No concurrency management: parallel calls spawn parallel processes.
pub struct Executor {
    backend: Arc<dyn CliBackend>,
}

impl Executor {
    pub fn new(backend: Arc<dyn CliBackend>) -> Self {
        Self { backend }
    }

    /// Spawn the CLI for `prompt` in the given workspace, respecting `session` state.
    pub async fn run(
        &self,
        prompt: &str,
        workspace: &WorkspaceHandle,
        session: &SessionState,
    ) -> Result<CliResponse> {
        let mut cmd = self
            .backend
            .build_command(prompt, &workspace.directory, session);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let start = std::time::Instant::now();

        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to spawn `{}` CLI", self.backend.name()))?;

        let stdout_handle = child.stdout.take().expect("stdout is piped");
        let stderr_handle = child.stderr.take().expect("stderr is piped");

        let (stdout_bytes, stderr_bytes) =
            tokio::join!(read_to_end(stdout_handle), read_to_end(stderr_handle),);

        let status = child
            .wait()
            .await
            .context("failed to wait on CLI process")?;
        let duration = start.elapsed();

        let exit_code = status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&stdout_bytes?).into_owned();
        let stderr = String::from_utf8_lossy(&stderr_bytes?).into_owned();

        Ok(self
            .backend
            .parse_output(stdout, stderr, exit_code, duration))
    }
}

async fn read_to_end<R: tokio::io::AsyncRead + Unpin>(mut reader: R) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    reader
        .read_to_end(&mut buf)
        .await
        .context("failed to read subprocess output")?;
    Ok(buf)
}

#[cfg(test)]
#[path = "tests/executor_test.rs"]
mod tests;
