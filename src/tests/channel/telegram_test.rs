use super::*;
use crate::types::AllowedUser;

#[test]
fn resolve_users_normalizes_handle_with_at() {
    // "@MyUser" should be stored as "myuser" — no '@', lowercased.
    let users = vec![AllowedUser::Handle("@MyUser".to_string())];
    let resolved = resolve_users(&users).unwrap();
    assert!(resolved.contains("myuser"));
    assert!(!resolved.contains("@MyUser"));
    assert!(!resolved.contains("@myuser"));
}

#[test]
fn resolve_users_normalizes_handle_without_at() {
    let users = vec![AllowedUser::Handle("SomeUser".to_string())];
    let resolved = resolve_users(&users).unwrap();
    assert!(resolved.contains("someuser"));
}

#[test]
fn resolve_users_keeps_numeric_id() {
    let users = vec![AllowedUser::NumericId(987654321)];
    let resolved = resolve_users(&users).unwrap();
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
