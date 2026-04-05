use super::*;

#[test]
fn all_backends_are_buildable() {
    for name in ["claude-cli", "codex-cli", "gemini-cli"] {
        let b = build(name).unwrap_or_else(|_| panic!("build({name}) failed"));
        assert_eq!(b.name(), name);
    }
}

#[test]
fn unknown_backend_returns_error() {
    assert!(build("nonexistent-cli").is_err());
}
