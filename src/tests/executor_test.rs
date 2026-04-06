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
        timeout: None,
    }
}

fn workspace_with_timeout(timeout: Duration) -> WorkspaceHandle {
    WorkspaceHandle {
        name: "test".to_string(),
        directory: std::path::PathBuf::from("/tmp"),
        backend: "mock".to_string(),
        timeout: Some(timeout),
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

#[tokio::test]
async fn timeout_kills_slow_process() {
    struct SlowBackend;
    impl CliBackend for SlowBackend {
        fn build_command(
            &self,
            _: &str,
            working_dir: &Path,
            _: &SessionState,
        ) -> tokio::process::Command {
            let mut cmd = tokio::process::Command::new("sh");
            cmd.args(["-c", "sleep 60"]);
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
            "slow"
        }
    }

    let exec = Executor::new(Arc::new(SlowBackend));
    let err = exec
        .run(
            "x",
            &workspace_with_timeout(Duration::from_millis(200)),
            &SessionState::new(),
        )
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("timed out"),
        "expected timeout error, got: {err}"
    );
}

#[tokio::test]
async fn no_timeout_configured_fast_command_succeeds() {
    let exec = Executor::new(Arc::new(MockBackend { exit_code: 0 }));
    // No timeout — DEFAULT_TIMEOUT applies (10 min), fast command should not trigger it.
    let resp = exec
        .run("hi", &workspace(), &SessionState::new())
        .await
        .unwrap();
    assert_eq!(resp.exit_code, 0);
}

#[tokio::test]
async fn timeout_not_triggered_by_fast_command() {
    let exec = Executor::new(Arc::new(MockBackend { exit_code: 0 }));
    // Very short timeout but the command is fast.
    let resp = exec
        .run(
            "hi",
            &workspace_with_timeout(Duration::from_secs(5)),
            &SessionState::new(),
        )
        .await
        .unwrap();
    assert_eq!(resp.exit_code, 0);
}
