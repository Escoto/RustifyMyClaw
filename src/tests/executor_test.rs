use super::*;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::backend::CliBackend;
use crate::types::{CliResponse, SessionState, WorkspaceHandle};

/// A mock backend that runs `echo` so tests don't need a real CLI.
struct MockBackend {
    exit_code: i32,
}

impl CliBackend for MockBackend {
    fn build_command(
        &self,
        prompt: &str,
        working_dir: &Path,
        _session: &SessionState,
    ) -> tokio::process::Command {
        let mut cmd = tokio::process::Command::new("echo");
        cmd.arg(format!("mock: {prompt}"));
        cmd.current_dir(working_dir);
        cmd.stdin(std::process::Stdio::null());
        cmd
    }

    fn parse_output(
        &self,
        stdout: String,
        stderr: String,
        _exit_code: i32,
        duration: Duration,
    ) -> CliResponse {
        CliResponse {
            stdout,
            stderr,
            exit_code: self.exit_code,
            duration,
        }
    }

    fn name(&self) -> &'static str {
        "mock"
    }
}

fn workspace() -> WorkspaceHandle {
    WorkspaceHandle {
        name: "test".to_string(),
        directory: std::path::PathBuf::from("/tmp"),
        backend: "mock".to_string(),
    }
}

#[tokio::test]
async fn successful_run_captures_stdout() {
    let exec = Executor::new(Arc::new(MockBackend { exit_code: 0 }));
    let resp = exec
        .run("hello", &workspace(), &SessionState::new())
        .await
        .unwrap();
    assert!(resp.stdout.contains("mock: hello"));
    assert_eq!(resp.exit_code, 0);
}

#[tokio::test]
async fn nonzero_exit_code_is_captured() {
    struct FailBackend;
    impl CliBackend for FailBackend {
        fn build_command(
            &self,
            _: &str,
            working_dir: &Path,
            _: &SessionState,
        ) -> tokio::process::Command {
            let mut cmd = tokio::process::Command::new("sh");
            cmd.args(["-c", "echo fail_out; echo fail_err >&2; exit 42"]);
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
            "fail"
        }
    }

    let exec = Executor::new(Arc::new(FailBackend));
    let resp = exec
        .run("x", &workspace(), &SessionState::new())
        .await
        .unwrap();
    assert_eq!(resp.exit_code, 42);
    assert!(resp.stdout.contains("fail_out"));
    assert!(resp.stderr.contains("fail_err"));
}

#[tokio::test]
async fn duration_is_populated() {
    let exec = Executor::new(Arc::new(MockBackend { exit_code: 0 }));
    let resp = exec
        .run("hello", &workspace(), &SessionState::new())
        .await
        .unwrap();
    assert!(resp.duration.as_nanos() > 0);
}
