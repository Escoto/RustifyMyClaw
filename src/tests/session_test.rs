use super::*;
use crate::types::ChannelKind;

fn tg_chat(id: &str) -> ChatId {
    ChatId {
        channel: ChannelKind::Telegram,
        platform_id: id.to_string(),
    }
}

fn wa_chat(id: &str) -> ChatId {
    ChatId {
        channel: ChannelKind::WhatsApp,
        platform_id: id.to_string(),
    }
}

#[test]
fn fresh_chat_is_not_active() {
    let store = SessionStore::new();
    assert!(!store.should_continue(&tg_chat("42")));
}

#[test]
fn after_mark_active_should_continue() {
    let mut store = SessionStore::new();
    let id = tg_chat("42");
    store.mark_active(&id);
    assert!(store.should_continue(&id));
}

#[test]
fn after_reset_is_not_active() {
    let mut store = SessionStore::new();
    let id = tg_chat("42");
    store.mark_active(&id);
    store.reset(&id);
    assert!(!store.should_continue(&id));
}

#[test]
fn different_platforms_same_id_are_independent() {
    let mut store = SessionStore::new();
    let tg = tg_chat("12345");
    let wa = wa_chat("12345");
    store.mark_active(&tg);
    assert!(store.should_continue(&tg));
    assert!(!store.should_continue(&wa));
}

#[test]
fn reset_nonexistent_is_noop() {
    let mut store = SessionStore::new();
    store.reset(&tg_chat("nonexistent")); // should not panic
}
