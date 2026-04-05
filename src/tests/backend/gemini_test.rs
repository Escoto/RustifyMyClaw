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
    assert_eq!(GeminiBackend.name(), "gemini-cli");
}

#[test]
fn binary_is_gemini() {
    let cmd = GeminiBackend.build_command("hello", Path::new("/tmp"), &fresh_session());
    assert_eq!(cmd.as_std().get_program(), "gemini");
}

#[test]
fn prompt_passed_after_p_flag() {
    let cmd = GeminiBackend.build_command("my prompt", Path::new("/tmp"), &fresh_session());
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
fn non_interactive_flag_present() {
    let cmd = GeminiBackend.build_command("x", Path::new("/tmp"), &fresh_session());
    let args: Vec<_> = cmd.as_std().get_args().collect();
    assert!(args.contains(&std::ffi::OsStr::new("-y")));
}

#[test]
fn session_state_ignored_no_continue_flag() {
    for session in [fresh_session(), active_session()] {
        let cmd = GeminiBackend.build_command("x", Path::new("/tmp"), &session);
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
    let resp = GeminiBackend.parse_output(
        "answer".to_string(),
        "".to_string(),
        0,
        Duration::from_millis(80),
    );
    assert_eq!(resp.stdout, "answer");
    assert_eq!(resp.exit_code, 0);
}
