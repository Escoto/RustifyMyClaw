use super::*;
use crate::types::SessionState;
use std::path::Path;

fn fresh_session() -> SessionState {
    SessionState::new()
}

fn active_session() -> SessionState {
    let mut s = SessionState::new();
    s.is_active = true;
    s
}

#[test]
fn name_is_correct() {
    assert_eq!(ClaudeCodeBackend.name(), "claude-cli");
}

#[test]
fn fresh_session_has_no_continue_flag() {
    let cmd = ClaudeCodeBackend.build_command("hello", Path::new("/tmp"), &fresh_session());
    let args: Vec<_> = cmd.as_std().get_args().collect();
    assert!(!args.contains(&std::ffi::OsStr::new("-c")));
    assert!(args.contains(&std::ffi::OsStr::new("-p")));
}

#[test]
fn active_session_has_continue_flag() {
    let cmd = ClaudeCodeBackend.build_command("hello", Path::new("/tmp"), &active_session());
    let args: Vec<_> = cmd.as_std().get_args().collect();
    assert!(args.contains(&std::ffi::OsStr::new("-c")));
}

#[test]
fn prompt_is_passed_after_p_flag() {
    let cmd = ClaudeCodeBackend.build_command("my prompt", Path::new("/tmp"), &fresh_session());
    let args: Vec<_> = cmd
        .as_std()
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let p_idx = args
        .iter()
        .position(|a| a == "-p")
        .expect("-p flag present");
    assert_eq!(args[p_idx + 1], "my prompt");
}

#[test]
fn parse_output_passthrough() {
    use std::time::Duration;
    let resp = ClaudeCodeBackend.parse_output(
        "output".to_string(),
        "".to_string(),
        0,
        Duration::from_millis(100),
    );
    assert_eq!(resp.stdout, "output");
    assert_eq!(resp.exit_code, 0);
}
