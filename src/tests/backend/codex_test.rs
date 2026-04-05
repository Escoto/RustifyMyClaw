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
    assert_eq!(CodexBackend.name(), "codex-cli");
}

#[test]
fn binary_is_codex() {
    let cmd = CodexBackend.build_command("hello", Path::new("/tmp"), &fresh_session());
    assert_eq!(cmd.as_std().get_program(), "codex");
}

#[test]
fn prompt_is_passed_as_arg() {
    let cmd = CodexBackend.build_command("my prompt", Path::new("/tmp"), &fresh_session());
    let args: Vec<_> = cmd
        .as_std()
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert!(args.contains(&"my prompt".to_string()));
}

#[test]
fn quiet_flag_present() {
    let cmd = CodexBackend.build_command("x", Path::new("/tmp"), &fresh_session());
    let args: Vec<_> = cmd.as_std().get_args().collect();
    assert!(args.contains(&std::ffi::OsStr::new("-q")));
}

#[test]
fn session_state_ignored_no_continue_flag() {
    for session in [fresh_session(), active_session()] {
        let cmd = CodexBackend.build_command("x", Path::new("/tmp"), &session);
        let args: Vec<_> = cmd
            .as_std()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(
            !args.contains(&"--continue".to_string()) && !args.contains(&"-c".to_string()),
            "unexpected continue flag in args: {args:?}"
        );
    }
}

#[test]
fn parse_output_passthrough() {
    use std::time::Duration;
    let resp = CodexBackend.parse_output(
        "result".to_string(),
        "".to_string(),
        0,
        Duration::from_millis(50),
    );
    assert_eq!(resp.stdout, "result");
    assert_eq!(resp.exit_code, 0);
}
