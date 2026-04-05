/// Parsed user intent from a raw message string.
#[derive(Debug, PartialEq)]
pub enum BridgeCommand {
    /// Plain text prompt to forward to the CLI backend.
    Prompt { text: String },
    /// `/new` — reset the current session.
    NewSession,
    /// `/status` — report workspace, backend, and session state.
    Status,
    /// `/help` — list available commands.
    Help,
    /// `/use <name>` — switch to a different workspace at runtime.
    UseWorkspace { name: String },
}

impl BridgeCommand {
    /// Parse a raw message string into a `BridgeCommand`.
    ///
    /// Leading and trailing whitespace is trimmed before matching.
    /// Empty or whitespace-only input produces `Prompt { text: "" }` — the executor
    /// will handle the empty-prompt case gracefully.
    pub fn parse(text: &str) -> Self {
        let trimmed = text.trim();
        match trimmed {
            "/new" => BridgeCommand::NewSession,
            "/status" => BridgeCommand::Status,
            "/help" => BridgeCommand::Help,
            other if other.starts_with("/use ") => {
                let name = other["/use ".len()..].trim().to_string();
                BridgeCommand::UseWorkspace { name }
            }
            other => BridgeCommand::Prompt {
                text: other.to_string(),
            },
        }
    }
}

#[cfg(test)]
#[path = "../tests/command_test.rs"]
mod tests;
