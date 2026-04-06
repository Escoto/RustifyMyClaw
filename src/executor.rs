use std::sync::Arc;

use anyhow::{bail, Context, Result};
use tokio::io::AsyncReadExt;

use crate::backend::CliBackend;
use crate::types::{CliResponse, SessionState, WorkspaceHandle};

/// Default CLI timeout when no `timeout` is configured on the workspace.
///
/// Set to a generous 10 minutes so the daemon does not silently hang forever while
/// still providing an upper bound for runaway processes.
const DEFAULT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

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
    ///
    /// If `workspace.timeout` is set, the process is killed and an error is returned
    /// if it does not complete within that duration. The default timeout is
    /// `DEFAULT_TIMEOUT` (10 minutes).
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

        let timeout_dur = workspace.timeout.unwrap_or(DEFAULT_TIMEOUT);

        // Race the pipe-draining future against the timeout timer.
        // The timeout must cover pipe reading too — a runaway process that never closes
        // stdout/stderr would otherwise block read_to_end forever before we reach wait().
        let pipe_result = tokio::select! {
            bytes = async {
                tokio::join!(read_to_end(stdout_handle), read_to_end(stderr_handle))
            } => Some(bytes),
            _ = tokio::time::sleep(timeout_dur) => None,
        };

        let (stdout_bytes, stderr_bytes) = match pipe_result {
            Some(pair) => pair,
            None => {
                child.kill().await.ok();
                bail!("CLI process timed out after {}s", timeout_dur.as_secs());
            }
        };

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
