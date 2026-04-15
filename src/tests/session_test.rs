use super::*;

#[test]
fn fresh_chat_is_not_active() {
    let store = SessionStore::new();
    assert!(!store.get(&ChatId::telegram("42")).is_active);
}

#[test]
fn after_mark_active_should_continue() {
    let mut store = SessionStore::new();
    let id = ChatId::telegram("42");
    store.mark_active(&id);
    assert!(store.get(&id).is_active);
}

#[test]
fn after_reset_is_not_active() {
    let mut store = SessionStore::new();
    let id = ChatId::telegram("42");
    store.mark_active(&id);
    store.reset(&id);
    assert!(!store.get(&id).is_active);
}

#[test]
fn different_platforms_same_id_are_independent() {
    let mut store = SessionStore::new();
    let tg = ChatId::telegram("12345");
    let wa = ChatId::whatsapp("12345");
    store.mark_active(&tg);
    assert!(store.get(&tg).is_active);
    assert!(!store.get(&wa).is_active);
}

#[test]
fn reset_nonexistent_is_noop() {
    let mut store = SessionStore::new();
    store.reset(&ChatId::telegram("nonexistent")); // should not panic
}
