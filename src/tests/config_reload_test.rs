use tokio_util::sync::CancellationToken;

use super::*;

/// Verifies that `watch` exits cleanly when the shutdown token is already cancelled.
///
/// This test uses `/tmp` (always present) so the file watcher initialises successfully.
/// The pre-cancelled token causes the select loop to break immediately without waiting
/// for any file-system events.
#[tokio::test]
async fn watch_returns_ok_on_immediate_shutdown() {
    let token = CancellationToken::new();
    token.cancel(); // cancelled before watch even starts its loop

    let result = watch(
        std::path::PathBuf::from("/tmp"),
        token,
        |_config| { /* no-op reload handler */ },
    )
    .await;

    assert!(result.is_ok(), "expected Ok(()), got: {:?}", result.err());
}

/// Verifies that `watch` returns an error when given a non-existent path.
#[tokio::test]
async fn watch_returns_error_for_nonexistent_path() {
    let token = CancellationToken::new();
    // Cancel immediately so we don't block waiting on events — but the watcher
    // creation should fail before we even reach the select loop.
    token.cancel();

    let result = watch(
        std::path::PathBuf::from("/nonexistent/path/config.yaml"),
        token,
        |_config| {},
    )
    .await;

    assert!(
        result.is_err(),
        "expected an error for nonexistent path, got Ok"
    );
}
