# BridgeCLI — Architecture Reference

> **Version:** 2.1 — Updated post-Phase 1 implementation to reflect actual codebase
> **Purpose:** Single source of truth for types, boundaries, and design decisions. Phase-specific build guides reference this document.

---

## 1. What This Is

A Rust daemon that bridges messaging platforms (Telegram, WhatsApp, Slack) to local AI CLI tools (Claude Code, Codex, Gemini CLI). Each workspace binds one or more messaging channels to a local directory and CLI backend. The daemon is a **dumb pipe** — messages in, CLI call, output back. No web UI, no PTY, no concurrency management.

```
┌──────────────┐
│  TG Bot A    │──┐
│  @coach_bot  │  │     ┌──────────────────────────────────────────┐     ┌──────────────┐
└──────────────┘  ├────▶│              BridgeCLI                   │────▶│  claude -p   │
┌──────────────┐  │     │  Listener → Router → Executor → Output  │◀────│  codex       │
│  WhatsApp    │──┤     └──────────────────────────────────────────┘     │  gemini      │
│  Channel     │  │                     ▲                               └──────────────┘
└──────────────┘  │                     │
┌──────────────┐  │               config.yaml
│  Slack       │──┘         ~/.rustifymyclaw/
│  Channel     │
└──────────────┘
```

### Core Principles

| Principle | Rule |
|-----------|------|
| Dumb pipe | No concurrency management — CLI backends own their own locking |
| Workspace-bound channels | Each bot/channel is bound to a workspace; no ambiguous routing |
| Admin-only config | Server owner edits YAML manually; no self-service |
| Silent security | Unauthorized messages silently dropped |
| V2-ready | Workspace reference is swappable for future `/use` command |

---

## 2. Canonical Types

Defined in `src/types.rs`. These are the contracts between components. Every module that handles messages, sessions, or identity must use these exact types.

### ChatId — Platform-Agnostic Identity

```rust
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ChatId {
    pub channel: ChannelKind,
    pub platform_id: String,
}
```

`String` for `platform_id` accommodates all platforms: Telegram `i64` (stringified), WhatsApp phone numbers, Slack alphanumeric IDs. Channel adapters convert their native type to `String`.

### ChannelKind

```rust
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum ChannelKind {
    Telegram,
    WhatsApp,
    Slack,
}
```

### AllowedUser

```rust
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum AllowedUser {
    NumericId(i64),      // Telegram numeric user ID
    Handle(String),      // @username, phone number, or Slack handle
}
```

### InboundMessage

```rust
pub struct InboundMessage {
    pub chat_id: ChatId,
    pub user_id: String,
    pub text: String,
    pub timestamp: DateTime<Utc>,
    pub context: MessageContext,
}
```

### MessageContext — Routing Without Lookup Tables

```rust
pub struct MessageContext {
    pub workspace: Arc<WorkspaceHandle>,
    pub provider: Arc<dyn ChannelProvider>,
}
```

The channel listener stamps every message with its `MessageContext` at ingestion time. The listener already knows its workspace and its own provider reference — so the router never needs a lookup table. This was a design decision resolved during Phase 1 implementation.

### CliResponse

```rust
pub struct CliResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration: Duration,
}
```

### SessionState

```rust
pub struct SessionState {
    pub is_active: bool,
    pub last_activity: DateTime<Utc>,  // not used in Phase 1; reserved for Phase 4 idle timeouts
}
```

### FormattedResponse / ResponseChunk

```rust
pub struct FormattedResponse {
    pub chunks: Vec<ResponseChunk>,
}

pub enum ResponseChunk {
    Text(String),
    File { filename: String, content: Vec<u8> },
}
```

---

## 3. Configuration

**Location:** `~/.rustifymyclaw/config.yaml`

```yaml
workspaces:
  - name: "super-project"
    directory: "/home/user-x/projects/super_project"    # MANDATORY
    backend: "claude-cli"                               # MANDATORY — claude-cli | codex-cli | gemini-cli
    channels:
      - kind: telegram                                  # MANDATORY
        bot_name: "@my_bot_bot"                         # OPTIONAL — display/logging only
        token: "${my_bot_TOKEN}"                        # MANDATORY — env var or hardcoded
        allowed_users:                                  # MANDATORY
          - "@user-x"
          - 987654321
        max_message_chars: 3500                         # OPTIONAL — overrides global output.max_message_chars

      - kind: whatsapp
        token: "${WHATSAPP_API_TOKEN}"                  # MANDATORY — Meta Graph API token
        phone_number_id: "${WA_PHONE_NUMBER_ID}"        # MANDATORY (WhatsApp) — Meta Business phone number ID
        webhook_port: 8080                              # OPTIONAL (WhatsApp) — port for inbound webhook server (default 8080)
        verify_token: "${WA_VERIFY_TOKEN}"              # OPTIONAL (WhatsApp) — webhook verification token
        allowed_users:
          - "+5511999999999"

      - kind: slack
        token: "${SLACK_BOT_TOKEN}"                     # MANDATORY — xoxb-* bot token for Web API
        app_token: "${SLACK_APP_TOKEN}"                 # MANDATORY (Slack) — xapp-* Socket Mode token
        use_threads: true                               # OPTIONAL (Slack) — reply in-thread instead of top-level (default false)
        max_message_chars: 3000                         # OPTIONAL — Slack renders poorly above ~3000 chars
        allowed_users:
          - "@dev_user"
          - "U01ABC123"                                 # raw Slack user ID also accepted

  - name: "data-pipeline"
    directory: "/home/user-x/projects/pipeline"
    backend: "codex-cli"
    channels:
      - kind: telegram
        bot_name: "@pipeline_bot"
        token: "${PIPELINE_BOT_TOKEN}"
        allowed_users:
          - "@user-x"
          - "@colleague_dev"

output:
  max_message_chars: 4000
  file_upload_threshold_bytes: 51200
  chunk_strategy: "natural"                             # "natural" | "fixed"
```

### Config Structs

```rust
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub workspaces: Vec<WorkspaceConfig>,
    pub output: OutputConfig,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceConfig {
    pub name: String,
    pub directory: PathBuf,
    pub backend: String,
    pub channels: Vec<ChannelConfig>,
}

#[derive(Debug, Deserialize)]
pub struct ChannelConfig {
    pub kind: String,
    pub bot_name: Option<String>,          // OPTIONAL — display/logging only
    pub token: String,
    pub allowed_users: Vec<AllowedUser>,

    // Per-channel output overrides — both fall back to global OutputConfig when absent.
    pub max_message_chars: Option<usize>,
    pub file_upload_threshold_bytes: Option<usize>,

    // WhatsApp-specific (ignored with a startup warning on non-whatsapp channels).
    pub phone_number_id: Option<String>,   // MANDATORY for whatsapp — Meta Business phone number ID
    pub webhook_port: Option<u16>,         // default 8080
    pub verify_token: Option<String>,      // webhook verification secret

    // Slack-specific (ignored with a startup warning on non-slack channels).
    pub app_token: Option<String>,         // MANDATORY for slack — xapp-* Socket Mode token
    pub use_threads: Option<bool>,         // default false — reply in-thread vs top-level
}

#[derive(Debug, Clone, Deserialize)]
pub struct OutputConfig {
    pub max_message_chars: usize,
    pub file_upload_threshold_bytes: usize,
    pub chunk_strategy: ChunkStrategy,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum ChunkStrategy {
    #[serde(rename = "natural")]
    Natural,
    #[serde(rename = "fixed")]
    Fixed,
}
```

**Env var interpolation:** The config loader replaces `${VAR_NAME}` patterns with `std::env::var("VAR_NAME")` at parse time via simple string parsing (find `${`, find `}`, extract, replace). No regex dependency.

**Misplaced field warnings:** `validate()` calls `warn_misplaced_fields()` for each channel. If a platform-specific field (e.g. `phone_number_id`) appears on the wrong channel kind, a `tracing::warn!` is emitted at startup. The field is silently ignored at runtime — validation does not bail — but the operator gets a clear signal rather than a silent no-op.

**`use_threads` (Slack-only):** The only channel-specific UX flag in the codebase. When `true`, `SlackProvider` sends responses as thread replies under the original message (`thread_ts` stored in an internal `RwLock<HashMap<platform_id, ts>>`). Telegram and WhatsApp have no equivalent; the field is meaningless and warned on those platforms.

---

## 4. Component Boundaries & Ownership

### Who Owns What

| Component | Owns | Shared Via |
|-----------|------|-----------|
| `ChannelListener` | One `ChannelProvider`, one `SecurityGate` | — (not shared) |
| `SecurityGate` | One `HashSet<String>` of resolved user IDs | — (one per channel, not shared) |
| `SessionStore` | All `ChatId → SessionState` mappings | `Arc<RwLock<SessionStore>>` |
| `WorkspaceHandle` | `name`, `directory`, `backend` (looked up from registry) | `Arc<WorkspaceHandle>` (V1), `Arc<RwLock<WorkspaceHandle>>` (V2) |

### Security Gate Scope: Per-Channel

Each channel gets its **own `SecurityGate`** instance, built from that channel's `allowed_users` list.

```
Workspace "super-project"
├── TelegramProvider → SecurityGate { allowed: ["123456", "987654321"] }
├── WhatsAppProvider → SecurityGate { allowed: ["+5511999999999"] }
```

### Session Store: Keyed by ChatId

Sessions are keyed by `ChatId` (which includes `ChannelKind`). Telegram chat `12345` and WhatsApp chat `12345` are different sessions — no collisions.

---

## 5. Design Patterns

### Strategy → CLI Backends

Each backend implements the `CliBackend` trait:

```rust
#[async_trait]
pub trait CliBackend: Send + Sync {
    fn build_command(&self, prompt: &str, working_dir: &Path, session: &SessionState) -> Command;
    fn parse_output(&self, raw: &str) -> CliResponse;
    fn name(&self) -> &'static str;
}
```

### Backend Registry (Implemented)

`backend/mod.rs` exposes a `build(name) -> Result<Box<dyn CliBackend>>` factory. `main.rs` calls it once per distinct backend name at startup, wraps each result in `Arc`, and stores them in a `HashMap<String, Arc<dyn CliBackend>>` that is passed directly to `Router::new()`. The router owns the map and looks up backends by the workspace's `backend` string.

```rust
// backend/mod.rs
pub fn build(backend_name: &str) -> Result<Box<dyn CliBackend>> {
    match backend_name {
        "claude-cli" => Ok(Box::new(ClaudeCodeBackend)),
        "codex-cli"  => Ok(Box::new(CodexBackend)),
        "gemini-cli" => Ok(Box::new(GeminiBackend)),
        other        => bail!("unknown backend: `{other}`"),
    }
}

// main.rs (startup)
if !backends.contains_key(&ws_config.backend) {
    let b = backend::build(&ws_config.backend)?;
    backends.insert(ws_config.backend.clone(), Arc::from(b));
}
```

Each backend is instantiated at most once regardless of how many workspaces share it. `Arc` sharing means the router never allocates a new backend per message.

### Adapter → Channel Providers

```rust
#[async_trait]
pub trait ChannelProvider: Send + Sync {
    async fn start(self: Arc<Self>, tx: mpsc::Sender<InboundMessage>) -> Result<()>;
    async fn send_response(&self, chat_id: &ChatId, response: FormattedResponse) -> Result<()>;
    async fn resolve_users(&self, users: &[AllowedUser]) -> Result<HashSet<String>>;
}
```

**Note the `self: Arc<Self>` on `start()`.** This is the self-arc pattern adopted during Phase 1. The `TelegramProvider` needs to pass a reference to itself into the teloxide `repl()` closure so it can stamp `MessageContext` on every inbound message. A `&self` borrow doesn't work because the closure outlives the function call. `Arc<Self>` gives the closure shared ownership.

This pattern applies to all future channel providers. When implementing WhatsApp or Slack providers in Phase 3, `start()` will use the same `self: Arc<Self>` signature.

### Command → User Actions

```rust
pub enum BridgeCommand {
    Prompt { text: String },
    NewSession,
    Status,
    Help,
    // V2: UseWorkspace { name: String },
}
```

---

## 6. Pipeline & Router Role

```
ChannelProvider ──mpsc──▶ Router ──mpsc──▶ Executor ──mpsc──▶ Responder
                           │                  │                   │
                      parse command       CliBackend          send_response
                      (workspace +        (Strategy)          (via MessageContext
                       provider come                           .provider)
                       from MessageContext)
```

### What the Router Actually Does

1. **Parse command** — `BridgeCommand::parse(msg.text)`
2. **Handle non-prompt commands** — `/new`, `/status`, `/help` short-circuit without hitting the executor. Response is sent back via `msg.context.provider`.
3. **Prepare execution context** — For `Prompt`: read session state, get backend from `msg.context.workspace`, bundle into execution request.
4. **Post-execution bookkeeping** — Mark session active, format response, send back via `msg.context.provider`.

The router does **not** maintain a workspace lookup table. The `MessageContext` on each `InboundMessage` already carries the workspace and provider references, stamped by the channel listener at ingestion time.

---

## 7. Output Formatting

### Chunk Strategies

**`Natural`** (implemented in Phase 1):
1. Code block boundaries (``` markers)
2. Paragraph breaks (`\n\n`)
3. Line breaks (`\n`)
4. Hard cut at `max_message_chars` with `char_boundary_floor()` for UTF-8 safety

**`Fixed`** (Phase 2):
Hard cut at `max_message_chars` with `char_boundary_floor()`. No boundary detection.

Both strategies produce chunks ≤ `max_message_chars`. If total output exceeds `file_upload_threshold_bytes`, skip chunking and upload as a file.

**`char_boundary_floor()`** — Added during Phase 1 to prevent panics when slicing multi-byte UTF-8 strings. Always rounds down to the nearest valid char boundary. All chunking code uses this instead of raw byte indexing.

### Error Output

If the CLI exits nonzero with non-empty stderr, prepend exit code and stderr to the response. The formatter handles this — the executor returns raw data.

---

## 8. V2 `/use` Preparation

V1 holds workspace as `Arc<WorkspaceHandle>`. V2 changes this to `Arc<RwLock<WorkspaceHandle>>`. The critical rule: **never hold a raw `WorkspaceHandle`** — always go through `Arc`. This is enforced in `CLAUDE.md`.

The `MessageContext.workspace` field is `Arc<WorkspaceHandle>` in V1. In V2, this becomes `Arc<RwLock<WorkspaceHandle>>`, and the router reads it via `.read()` lock. The channel listener's `start()` method clones the Arc for each message — in V2 it clones the `Arc<RwLock<>>` instead. Minimal diff.

---

## 9. Concurrency: Dumb Pipe

BridgeCLI does not manage CLI-level concurrency. Parallel prompts to the same workspace spawn parallel CLI processes. The CLI backend handles its own locking. BridgeCLI faithfully returns whatever the CLI outputs — success or lock error.

---

## 10. Project Structure

```
bridgecli/
├── Cargo.toml
├── CLAUDE.md
├── desired_architecture.md
├── src/
│   ├── main.rs
│   ├── types.rs                # ChatId, ChannelKind, InboundMessage, MessageContext,
│   │                           # CliResponse, SessionState, FormattedResponse, ResponseChunk
│   ├── config.rs               # AppConfig, YAML parsing, env var interpolation
│   ├── security.rs             # SecurityGate (per-channel)
│   ├── router.rs               # Orchestration hub: parse → session → execute → format → respond
│   ├── session.rs              # SessionStore keyed by ChatId
│   ├── executor.rs             # Dumb pipe: spawn CLI, capture output
│   ├── formatter.rs            # Chunking (natural + char_boundary_floor), file upload
│   │
│   ├── command/
│   │   └── mod.rs              # BridgeCommand enum + parse
│   │
│   ├── backend/
│   │   ├── mod.rs              # CliBackend trait + BackendRegistry
│   │   └── claude.rs           # ClaudeCodeBackend
│   │                           # Phase 2: codex.rs, gemini.rs
│   │
│   └── channel/
│       ├── mod.rs              # ChannelProvider trait (start takes Arc<Self>)
│       └── telegram.rs         # TelegramProvider (teloxide 0.17, polling mode)
│                               # Phase 3: whatsapp.rs, slack.rs
│
└── tests/
    ├── config_test.rs
    ├── command_test.rs
    ├── session_test.rs
    ├── security_test.rs
    ├── executor_test.rs
    └── formatter_test.rs
```

### Dependencies (Phase 1 Actual)

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
anyhow = "1"
thiserror = "1"
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
async-trait = "0.1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
chrono = { version = "0.4", features = ["serde"] }
teloxide = { version = "0.17", features = ["macros"] }
```

No `regex` dependency — env var interpolation uses simple string parsing.

---

## 11. Phased Delivery Overview

| Phase | Scope | Key Deliverables |
|-------|-------|-----------------|
| 1 ✅ | Foundation + Telegram + Claude Code | Types, config, security, session, executor, formatter, TG listener, wired pipeline. 50 tests passing. |
| 2 ✅ | Multi-backend + `/use` | Codex + Gemini backends, `/use` command, `Arc<RwLock>` workspace, `Fixed` chunking. 80 tests passing. |
| 3 ✅ | Multi-channel | WhatsApp (axum webhook) + Slack (Socket Mode) providers, per-channel output limits, misplaced-field warnings. 103 tests passing. |
| 4 | Hardening | Graceful shutdown, timeouts, rate limiting, config hot-reload, Windows |

---

## 12. Implementation Lessons from Phase 1

These are patterns that emerged during implementation and are now part of the project's conventions:

### Self-Arc Pattern for Channel Providers

`ChannelProvider::start()` takes `self: Arc<Self>` instead of `&self`. This is required because teloxide's `repl()` (and likely any future channel library's listen loop) takes a closure that must own its captures. The provider needs to reference itself inside the closure to stamp `MessageContext`. A borrow doesn't live long enough; `Arc` gives shared ownership.

All future channel providers must follow this pattern.

### Backend Registry over Factory Function

Backends are instantiated once at startup and stored in a `HashMap<String, Arc<dyn CliBackend>>`. The router looks up backends by name. This replaced the original factory-per-call design because multiple workspaces can share the same backend type, and `Arc` sharing is cheaper than repeated `Box` allocation.

When adding new backends, add a match arm to `build()` in `backend/mod.rs`.

### UTF-8 Safe Chunking

All string slicing in the formatter goes through `char_boundary_floor()` to prevent panics on multi-byte characters. This is not optional — any new chunking logic must use it.
