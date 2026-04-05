use std::collections::HashSet;

/// Guards a single channel. Unauthorized messages are silently dropped.
///
/// Each channel gets its own `SecurityGate` instance built from that channel's
/// `allowed_users` list. Identity resolution (username → ID) happens outside this
/// struct, in the channel provider's `resolve_users()` call.
#[derive(Clone)]
pub struct SecurityGate {
    allowed: HashSet<String>,
}

impl SecurityGate {
    /// Build a gate from a set of already-resolved user ID strings.
    pub fn new(allowed: HashSet<String>) -> Self {
        Self { allowed }
    }

    /// Returns `true` if the user is allowed to send messages through this channel.
    pub fn is_allowed(&self, user_id: &str) -> bool {
        self.allowed.contains(user_id)
    }
}

#[cfg(test)]
#[path = "tests/security_test.rs"]
mod tests;
