use std::collections::HashMap;

use chrono::Utc;

use crate::types::{ChatId, SessionState};

/// In-memory store mapping each conversation to its session state.
///
/// Keyed by `ChatId` (which encodes `ChannelKind`) so Telegram chat `12345`
/// and WhatsApp chat `12345` are distinct sessions.
pub struct SessionStore {
    sessions: HashMap<ChatId, SessionState>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Mark a session active after a successful prompt execution.
    pub fn mark_active(&mut self, chat_id: &ChatId) {
        let state = self.sessions.entry(chat_id.clone()).or_default();
        state.is_active = true;
        state.last_activity = Utc::now();
    }

    /// Reset a session (e.g. on `/new` command). Next prompt will start fresh.
    pub fn reset(&mut self, chat_id: &ChatId) {
        self.sessions.remove(chat_id);
    }

    /// Returns a copy of the current state for a chat, or a default if not seen before.
    pub fn get(&self, chat_id: &ChatId) -> SessionState {
        self.sessions
            .get(chat_id)
            .cloned()
            .unwrap_or_else(SessionState::new)
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "tests/session_test.rs"]
mod tests;
