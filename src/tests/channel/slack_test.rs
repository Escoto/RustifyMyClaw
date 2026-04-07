use super::*;
use crate::types::AllowedUser;

// ─── resolve_users (static logic, no network) ─────────────────────────────────

#[tokio::test]
async fn resolve_users_raw_user_id_passes_through() {
    let client = reqwest::Client::new();
    let users = vec![AllowedUser::Handle("U01ABC123".to_string())];
    let resolved = resolve_users(&users, "xoxb-unused", &client).await.unwrap();
    assert!(resolved.contains("U01ABC123"));
}

#[tokio::test]
async fn resolve_users_raw_workspaceid_passes_through() {
    let client = reqwest::Client::new();
    let users = vec![AllowedUser::Handle("W01XYZ".to_string())];
    let resolved = resolve_users(&users, "xoxb-unused", &client).await.unwrap();
    assert!(resolved.contains("W01XYZ"));
}

#[tokio::test]
async fn resolve_users_numeric_id_is_skipped_with_warning() {
    let client = reqwest::Client::new();
    let users = vec![AllowedUser::NumericId(99999)];
    // Numeric IDs are ignored; the set will be empty.
    let resolved = resolve_users(&users, "xoxb-unused", &client).await.unwrap();
    assert!(resolved.is_empty());
}

// ─── Socket Mode ack format ───────────────────────────────────────────────────

#[test]
fn ack_json_has_correct_shape() {
    let ack = build_ack("abc-123");
    let parsed: serde_json::Value = serde_json::from_str(&ack).unwrap();
    assert_eq!(parsed["envelope_id"], "abc-123");
}

#[test]
fn ack_json_has_only_envelope_id() {
    let ack = build_ack("xyz-456");
    let parsed: serde_json::Value = serde_json::from_str(&ack).unwrap();
    assert!(parsed.as_object().unwrap().len() == 1);
}

// ─── Socket Mode event payload parsing ───────────────────────────────────────

#[test]
fn socket_mode_envelope_deserializes_events_api() {
    let json = r#"{
        "type": "events_api",
        "envelope_id": "e1234",
        "payload": {
            "event": {
                "type": "message",
                "user": "U01ABC",
                "text": "hello bot",
                "channel": "C01XYZ",
                "ts": "1620000000.000100"
            }
        }
    }"#;
    let envelope: SocketModeEnvelope = serde_json::from_str(json).unwrap();
    assert_eq!(envelope.kind, "events_api");
    assert_eq!(envelope.envelope_id.as_deref(), Some("e1234"));
    let payload = envelope.payload.unwrap();
    assert_eq!(payload["event"]["user"], "U01ABC");
    assert_eq!(payload["event"]["text"], "hello bot");
}

#[test]
fn socket_mode_hello_envelope_deserializes() {
    let json = r#"{ "type": "hello" }"#;
    let envelope: SocketModeEnvelope = serde_json::from_str(json).unwrap();
    assert_eq!(envelope.kind, "hello");
    assert!(envelope.envelope_id.is_none());
}

#[test]
fn socket_mode_disconnect_envelope_deserializes() {
    let json = r#"{ "type": "disconnect", "reason": "warning" }"#;
    let envelope: SocketModeEnvelope = serde_json::from_str(json).unwrap();
    assert_eq!(envelope.kind, "disconnect");
}
