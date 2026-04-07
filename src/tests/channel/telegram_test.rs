use super::*;
use crate::channel::ChannelProvider;
use crate::config::{ChunkStrategy, OutputConfig};
use crate::security::SecurityGate;
use crate::types::{AllowedUser, WorkspaceHandle};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

fn make_provider() -> TelegramProvider {
    let workspace = Arc::new(RwLock::new(WorkspaceHandle {
        name: "test".to_string(),
        directory: PathBuf::from("/tmp"),
        backend: "claude-cli".to_string(),
        timeout: None,
    }));
    let output_config = Arc::new(OutputConfig {
        max_message_chars: 4000,
        file_upload_threshold_bytes: 51200,
        chunk_strategy: ChunkStrategy::Natural,
    });
    TelegramProvider::new(
        "fake-token".to_string(),
        SecurityGate::new(HashSet::new()),
        workspace,
        output_config,
    )
}

#[tokio::test]
async fn resolve_users_normalizes_handle_with_at() {
    // "@MyUser" should be stored as "myuser" — no '@', lowercased.
    let provider = make_provider();
    let users = vec![AllowedUser::Handle("@MyUser".to_string())];
    let resolved = provider.resolve_users(&users).await.unwrap();
    assert!(resolved.contains("myuser"));
    assert!(!resolved.contains("@MyUser"));
    assert!(!resolved.contains("@myuser"));
}

#[tokio::test]
async fn resolve_users_normalizes_handle_without_at() {
    let provider = make_provider();
    let users = vec![AllowedUser::Handle("SomeUser".to_string())];
    let resolved = provider.resolve_users(&users).await.unwrap();
    assert!(resolved.contains("someuser"));
}

#[tokio::test]
async fn resolve_users_keeps_numeric_id() {
    let provider = make_provider();
    let users = vec![AllowedUser::NumericId(987654321)];
    let resolved = provider.resolve_users(&users).await.unwrap();
    assert!(resolved.contains("987654321"));
}

#[test]
fn short_message_unchanged() {
    assert_eq!(enforce_telegram_limit("hello"), "hello");
}

#[test]
fn long_message_truncated_within_limit() {
    let msg = "a".repeat(5000);
    let result = enforce_telegram_limit(&msg);
    assert!(result.len() <= TELEGRAM_MAX_CHARS);
    assert!(result.ends_with(TRUNCATION_SUFFIX));
}

#[test]
fn unicode_boundary_respected() {
    // "こ" is 3 bytes — must not split mid-char
    let msg = "こ".repeat(2000); // 6000 bytes
    let result = enforce_telegram_limit(&msg);
    assert!(result.len() <= TELEGRAM_MAX_CHARS);
    assert!(std::str::from_utf8(result.as_bytes()).is_ok());
}

#[test]
fn exactly_at_limit_unchanged() {
    let msg = "a".repeat(TELEGRAM_MAX_CHARS);
    let result = enforce_telegram_limit(&msg);
    assert_eq!(result.len(), TELEGRAM_MAX_CHARS);
    assert!(!result.ends_with(TRUNCATION_SUFFIX));
}
