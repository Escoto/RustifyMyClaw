use super::*;

// ─── enforce_whatsapp_limit ───────────────────────────────────────────────────

#[test]
fn short_message_unchanged() {
    assert_eq!(enforce_whatsapp_limit("hello"), "hello");
}

#[test]
fn long_message_truncated_within_limit() {
    let msg = "a".repeat(5000);
    let result = enforce_whatsapp_limit(&msg);
    assert!(result.len() <= WHATSAPP_MAX_CHARS);
    assert!(result.ends_with(TRUNCATION_SUFFIX));
}

#[test]
fn unicode_boundary_respected() {
    // "こ" is 3 bytes — must not split mid-char
    let msg = "こ".repeat(2000); // 6000 bytes
    let result = enforce_whatsapp_limit(&msg);
    assert!(result.len() <= WHATSAPP_MAX_CHARS);
    assert!(std::str::from_utf8(result.as_bytes()).is_ok());
}

#[test]
fn exactly_at_limit_unchanged() {
    let msg = "a".repeat(WHATSAPP_MAX_CHARS);
    let result = enforce_whatsapp_limit(&msg);
    assert_eq!(result.len(), WHATSAPP_MAX_CHARS);
    assert!(!result.ends_with(TRUNCATION_SUFFIX));
}

// ─── resolve_users ────────────────────────────────────────────────────────────

#[test]
fn resolve_users_phone_numbers_pass_through() {
    use crate::types::AllowedUser;
    let users = vec![
        AllowedUser::Handle("+5511999999999".to_string()),
        AllowedUser::Handle("+14155551234".to_string()),
    ];
    let resolved = resolve_users(&users).unwrap();
    assert!(resolved.contains("+5511999999999"));
    assert!(resolved.contains("+14155551234"));
    assert_eq!(resolved.len(), 2);
}

#[test]
fn resolve_users_numeric_id_is_skipped_with_warning() {
    use crate::types::AllowedUser;
    let users = vec![AllowedUser::NumericId(12345)];
    // Should succeed but produce an empty set (numeric IDs are not valid WA identifiers).
    let resolved = resolve_users(&users).unwrap();
    assert!(resolved.is_empty());
}

// ─── Webhook payload parsing ──────────────────────────────────────────────────

#[test]
fn webhook_payload_deserializes_text_message() {
    let json = r#"{
        "entry": [{
            "changes": [{
                "value": {
                    "messages": [{
                        "from": "+5511999999999",
                        "type": "text",
                        "text": { "body": "Hello bot" }
                    }]
                }
            }]
        }]
    }"#;
    let payload: WebhookPayload = serde_json::from_str(json).unwrap();
    let msg = &payload.entry[0].changes[0].value.messages[0];
    assert_eq!(msg.from, "+5511999999999");
    assert_eq!(msg.kind, "text");
    assert_eq!(msg.text.as_ref().unwrap().body, "Hello bot");
}

#[test]
fn webhook_payload_empty_messages_is_valid() {
    let json = r#"{ "entry": [{ "changes": [{ "value": { "messages": [] } }] }] }"#;
    let payload: WebhookPayload = serde_json::from_str(json).unwrap();
    assert!(payload.entry[0].changes[0].value.messages.is_empty());
}

#[test]
fn webhook_payload_missing_messages_field_is_valid() {
    // The `messages` field is `#[serde(default)]` so it may be absent (e.g. status updates).
    let json = r#"{ "entry": [{ "changes": [{ "value": {} }] }] }"#;
    let payload: WebhookPayload = serde_json::from_str(json).unwrap();
    assert!(payload.entry[0].changes[0].value.messages.is_empty());
}
