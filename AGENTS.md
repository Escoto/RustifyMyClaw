# AGENTS.md

Instructions for AI coding agents working on RustifyMyClaw.

## Project overview

RustifyMyClaw is a Rust daemon (edition 2021, stable toolchain) that bridges messaging platforms (Telegram, WhatsApp, Slack) to local AI CLI tools (Claude Code, Codex, Gemini CLI). Each workspace binds one or more messaging channels to a local directory and a CLI backend. Messages arrive on a channel, flow through a fixed pipeline — security gate → router → executor → formatter → responder — and the CLI output goes back to the originating chat. No web UI, no database, no PTY.

## Build and test

```bash
cargo build                    # debug build
cargo build --release          # release build
cargo test                     # all tests (144 expected)
cargo test <module>            # single module, e.g. cargo test config
cargo clippy -- -D warnings    # zero warnings policy
cargo fmt --check              # format check
```

All four must pass before any commit.

## Module layout

| File | Purpose |
|------|---------|
| `src/main.rs` | Entrypoint. Loads config, calls orchestration helpers from `startup.rs`, reads like a table of contents. |
| `src/startup.rs` | Startup orchestration helpers: signal handler, rate limiter, workspace/backend/provider construction, router and provider spawning, config watcher, shutdown sequencing. |
| `src/types.rs` | All canonical types: `ChatId`, `ChannelKind`, `AllowedUser`, `InboundMessage`, `MessageContext`, `WorkspaceHandle`, `CliResponse`, `SessionState`, `FormattedResponse`, `ResponseChunk`. |
| `src/config.rs` | `AppConfig` struct hierarchy, YAML parsing, `${VAR}` env var interpolation, `resolve_path()` (config path resolution chain), `validate()`, `effective_output_config()`, `diff_reload()`, `warn_misplaced_fields()`. |
| `src/config_reload.rs` | `notify`-based file watcher. Debounced 300ms. Calls `load_from_path()` + `diff_reload()` + callback on change. |
| `src/security.rs` | `SecurityGate` — `HashSet<String>` of resolved user IDs, `is_allowed()` check. One instance per channel. |
| `src/session.rs` | `SessionStore` — `HashMap<ChatId, SessionState>`, `should_continue()`, `mark_active()`, `reset()`. |
| `src/router.rs` | Pipeline hub. Receives `InboundMessage` via mpsc, parses `BridgeCommand`, handles commands, dispatches prompts to executor, applies rate limiting, sends responses. |
| `src/executor.rs` | Spawns CLI via `tokio::process::Command`. Wraps stdout/stderr capture with timeout. Returns `CliResponse`. |
| `src/formatter.rs` | Chunks output (`Natural`/`Fixed` strategies). `char_boundary_floor()` for UTF-8 safety. File upload above threshold. |
| `src/rate_limit.rs` | Per-user sliding window. `HashMap<String, VecDeque<Instant>>` behind `Mutex`. Returns `Allowed` or `LimitedFor(Duration)`. |
| `src/command/mod.rs` | `BridgeCommand` enum: `Prompt`, `NewSession`, `Status`, `Help`, `UseWorkspace`. `parse()` method. |
| `src/backend/mod.rs` | `CliBackend` trait + `build()` factory. |
| `src/backend/claude.rs` | `ClaudeCodeBackend`: `claude -p "<prompt>"`, `-c` for session continuation. |
| `src/backend/codex.rs` | `CodexBackend`: `codex -q "<prompt>"`. No session continuation. |
| `src/backend/gemini.rs` | `GeminiBackend`: `gemini -p "<prompt>" -y`. No session continuation. |
| `src/channel/mod.rs` | `ChannelProvider` trait. |
| `src/channel/telegram.rs` | `TelegramProvider` — teloxide long-polling. |
| `src/channel/whatsapp.rs` | `WhatsAppProvider` — axum webhook + reqwest Graph API. |
| `src/channel/slack.rs` | `SlackProvider` — Socket Mode WebSocket via tokio-tungstenite. |
| `src/tests/` | All test files, referenced from source via `#[path = "tests/..."]`. |

## Key patterns

**Strategy — CLI backends.** `CliBackend` is the interface. `backend::build(name)` is the factory. `startup::build_workspaces()` builds one instance per distinct backend name, wraps each in `Arc`, and stores them in a `HashMap<String, Arc<dyn CliBackend>>`. The router looks up backends by name; it never allocates per-message.

**Adapter — channel providers.** `ChannelProvider` normalizes Telegram/WhatsApp/Slack behind a common interface. The router and executor only see `InboundMessage` and `FormattedResponse`.

**MessageContext stamping.** Channel providers stamp every `InboundMessage` with a `MessageContext` at ingestion time — workspace `Arc`, provider `Arc`, effective output config. The router reads routing info directly from the message; it holds no lookup tables.

**`Arc<WorkspaceHandle>` sharing.** Workspace handles are behind `Arc`. The `/use` command swaps the handle inside the `Arc<RwLock<>>` wrapper so listeners don't need to be restarted.

**`self_arc` pattern on `start()`.** `ChannelProvider::start()` takes a separate `self_arc: Arc<dyn ChannelProvider>` argument because polling closures need owned captures of the provider. A `&self` borrow doesn't live long enough.

## Common mistakes

- `unwrap()` or `expect()` in library code — use `?` with `anyhow::Result` or a `thiserror` enum.
- `println!` — use `tracing::{info, warn, error, debug, trace}` everywhere.
- `unsafe` — not permitted without a documented justification and review.
- `.clone()` to satisfy the borrow checker — redesign ownership or reach for `Arc`.
- `std::process::Command` — use `tokio::process::Command` for all subprocess calls.
- `std::thread::sleep` in async code — use `tokio::time::sleep`.
- Passing concrete types across module boundaries — use the trait (`CliBackend`, `ChannelProvider`).
- Holding a raw `WorkspaceHandle` in a listener — always go through `Arc`.

## Testing rules

- Unit tests: `#[cfg(test)] mod tests` block inside each source file, referencing `src/tests/<module>_test.rs` via `#[path = "..."]`.
- Every public function has at least one test.
- Never call a real CLI in tests — use `MockBackend`.
- No network or filesystem dependencies in tests. Mock everything external.
- Deterministic only — no sleep-based timing, no random data without seeds.
- Run the full suite with `cargo test` and confirm 144 tests pass before committing.

## File guide

| File | Purpose |
|------|---------|
| `CLAUDE.md` | Coding rules for this repo. Read before writing any code. |
| `docs/architecture.md` | Public-facing architecture reference. Start here to understand the design. |
| `docs/configuration.md` | Full `config.yaml` field reference. |
| `examples/config.yaml` | Minimal working config for quickstart. |
| `desired_architecture.md` | Internal planning history. Do not reference in public docs or PRs. |
| `CONTRIBUTING.md` | How to contribute — branch conventions, PR process, testing expectations. |
