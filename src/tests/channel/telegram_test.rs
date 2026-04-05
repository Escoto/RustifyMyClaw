use super::*;

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
