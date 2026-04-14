# CLAUDE.md — RustifyMyClaw

## Project

Rust daemon bridging messaging platforms to local AI CLI tools. See `docs/architecture.md` for architecture and component specs.

## Module Layout

| File | Purpose |
|------|---------|
| `src/main.rs` | Entrypoint. Loads config, calls orchestration helpers from `startup.rs`. |
| `src/startup.rs` | Startup orchestration: signal handler, rate limiter, workspace/backend/provider construction, router and provider spawning, config watcher, shutdown sequencing. |
| `src/types.rs` | All canonical types: `ChatId`, `ChannelKind`, `AllowedUser`, `InboundMessage`, `MessageContext`, `WorkspaceHandle`, `CliResponse`, `SessionState`, `FormattedResponse`, `ResponseChunk`. |
| `src/config.rs` | `AppConfig` struct hierarchy, YAML parsing, `${VAR}` env var interpolation, `resolve_path()`, `validate()`, `effective_output_config()`, `diff_reload()`, `warn_misplaced_fields()`. |
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
| `choco/` | Chocolatey package: `rustifymyclaw.nuspec`, install/uninstall scripts. |
| `.github/workflows/ci.yml` | CI: fmt, clippy, test on push/PR. |
| `.github/workflows/release.yml` | Release: version bump, cross-platform build, GH Release, then calls `publish.yml`. |
| `.github/workflows/publish.yml` | Reusable workflow (`workflow_call`): parallel crates.io + Chocolatey publish. |

## Build & Run

```bash
cargo build                    # debug build
cargo build --release          # release build
cargo test                     # all tests
cargo test <module>            # single module (e.g. cargo test config)
cargo clippy -- -D warnings    # lint — zero warnings policy
cargo fmt --check              # format check
```

All four must pass before any commit.

## Code Rules

### Rust Style

- **Edition 2021.** Target stable Rust, no nightly features.
- **`cargo fmt`** before every commit. No exceptions.
- **`cargo clippy -- -D warnings`** must pass clean. Treat all warnings as errors.
- **No `unwrap()` or `expect()` in library code.** Use `?` with `anyhow::Result` or `thiserror` enums. `unwrap()` is acceptable only in tests.
- **No `println!`** — use `tracing::{info, warn, error, debug, trace}` everywhere.
- **No `unsafe`** unless there is a documented, reviewed justification.
- **No `.clone()` to satisfy the borrow checker.** If you're cloning to fix a compile error, redesign the ownership. `Arc` and references exist for a reason. Cloning is fine for small, cheap types (strings from config, IDs).

### Architecture Compliance

- **Traits for boundaries.** Every major component boundary is a trait (`CliBackend`, `ChannelProvider`). Never pass concrete types across module boundaries.
- **One file per backend/channel.** `backend/claude.rs`, `backend/codex.rs`, `channel/telegram.rs`, etc. The `mod.rs` in each directory holds only the trait definition and factory function.
- **Config structs are dumb data.** No methods on config types beyond `Deserialize`. Logic lives in the components that consume them.
- **Workspace reference must stay behind `Arc`.** Never hold a raw `WorkspaceHandle` in a listener. This is critical for V2 `/use` compatibility.
- **Factory lookup, not per-message allocation.** `backend::build(name)` is the factory. `startup::build_workspaces()` builds one `Arc<dyn CliBackend>` per distinct backend name, stored in a `HashMap`. The router looks up by name.
- **MessageContext stamping at ingestion.** Channel providers stamp every `InboundMessage` with workspace `Arc`, provider `Arc`, and effective output config. The router reads routing info directly from the message — no lookup tables.
- **`self_arc` on `start()`.** `ChannelProvider::start()` takes a separate `self_arc: Arc<dyn ChannelProvider>` because polling closures need owned captures. A `&self` borrow doesn't live long enough.

### Error Handling

- **`thiserror`** for module-level error enums (typed, matchable).
- **`anyhow`** at the application boundary (`main.rs`, top-level orchestration).
- **Never swallow errors silently.** If you catch an error and don't propagate it, log it with `tracing::error!`.
- **Security gate is the one exception** — unauthorized messages are silently dropped with a `trace!`-level log (not `warn` or `error`).

### Async

- **Tokio runtime only.** No `async-std`, no `smol`.
- **`tokio::process::Command`** for all subprocess calls. Never use `std::process::Command`.
- **Never block the Tokio runtime.** No `std::thread::sleep`, no synchronous I/O in async contexts. Use `tokio::time::sleep`, `tokio::fs`, etc.
- **`mpsc` channels between pipeline stages.** Bounded channels with reasonable capacity (e.g., 64). If the channel is full, backpressure is correct — don't drop messages.

### Naming

- **Types:** `PascalCase` — `WorkspaceHandle`, `CliResponse`, `SecurityGate`
- **Functions/methods:** `snake_case` — `build_command`, `should_continue`, `parse_output`
- **Constants:** `SCREAMING_SNAKE_CASE` — `DEFAULT_CHUNK_SIZE`
- **Modules/files:** `snake_case` — `session.rs`, `formatter.rs`
- **Trait methods** describe what they return, not what they do: `fn name(&self)` not `fn get_name(&self)`

### Dependencies

- Add dependencies only when they solve a real problem. Justify each new crate in the PR.
- Pin major versions in `Cargo.toml` (e.g., `tokio = "1"`, `teloxide = "0.13"`).
- No feature flags unless explicitly needed. Start with defaults.

## Testing

### Requirements

- **Every public function has at least one test.** No exceptions.
- **Tests live in `tests/` for integration, `#[cfg(test)] mod tests` for unit.**
- **Use `MockBackend`** (see architecture doc) for executor tests — never call a real CLI in unit tests.
- **Test the edges:** empty input, max-length messages, unicode, messages with only whitespace, invalid YAML, missing env vars, unresolvable usernames.
- **No test should depend on network or filesystem state.** Mock everything external.
- **Tests must be deterministic.** No `sleep`-based timing, no random data without seeds.

### What to Test Per Module

| Module | Must Cover |
|--------|-----------|
| `config` | Valid YAML, missing fields, env var interpolation, unknown backend name, empty allowed_users |
| `security` | Allowed user passes, blocked user rejected, empty allowlist blocks all |
| `command` | `/new`, `/status`, `/help`, plain text, leading/trailing whitespace, empty string |
| `session` | Fresh → not continue, after prompt → continue, after `/new` → not continue |
| `backend/*` | Correct binary name, correct flags, `--continue` present/absent based on session |
| `executor` | Successful run, nonzero exit code, stderr capture, timeout (if implemented) |
| `formatter` | Under limit → single chunk, over limit → multiple chunks, over file threshold → file upload, code block preservation |

## Git

- **Commit messages:** imperative mood, concise. `Add session reset on /new command` not `Added stuff`.
- **One logical change per commit.** Don't mix refactors with features.
- **Branch naming:** `feat/<name>`, `fix/<name>`, `refactor/<name>`.

## How to Add a New Backend

1. Create `src/backend/<name>.rs` with a struct that implements `CliBackend`.
2. Implement `build_command()` — construct the `tokio::process::Command` with the correct binary and flags.
3. Implement `parse_output()` — for most CLI tools this is a pass-through that constructs `CliResponse` from the raw outputs.
4. Implement `name()` — return the config string identifier (e.g. `"newcli-cli"`).
5. Add `pub mod <name>;` in `src/backend/mod.rs`.
6. Add a match arm for the new name in `build()` in `src/backend/mod.rs`.
7. Add the name to `KNOWN_BACKENDS` in `src/config.rs`.
8. Add tests in `src/tests/backend/<name>_test.rs` and wire with `#[path = ...]` in the source file.
9. Update the backends table in `README.md` and the backend-specific notes in `docs/configuration.md`.

## How to Add a New Channel Provider

1. Create `src/channel/<name>.rs` with a struct that implements `ChannelProvider` and `ChannelProviderFactory`.
2. Add a module-level `resolve_users(users: &[AllowedUser]) -> Result<HashSet<String>>` function (or `async fn` if network I/O is needed) — convert `AllowedUser` entries to platform-native ID strings for `SecurityGate`. See `docs/architecture.md` for examples.
3. Implement `ChannelProviderFactory::create(ch_config, workspace, global_output)` — validate provider-specific config fields, call `resolve_users(&ch_config.allowed_users)`, build `SecurityGate::new(resolved)`, compute effective output config, then construct the provider once.
4. Implement `start(&self, tx, self_arc, shutdown)` — run the polling or webhook loop, check `SecurityGate`, stamp each message with `MessageContext`, send on `tx`. Exit when `shutdown.cancelled()` resolves. Use `self_arc` (not `self`) inside any closures that need to reference the provider.
5. Implement `send_response(&self, chat_id, response)` — deliver each `ResponseChunk` to the platform.
6. Stamp `MessageContext` at ingestion: `workspace: Arc<WorkspaceHandle>`, `provider: self_arc.clone()`, `output_config: Arc::new(effective_output_config(global, channel_cfg))`.
7. Add `pub mod <name>;` in `src/channel/mod.rs`.
8. Add the kind string to `KNOWN_CHANNELS` in `src/config.rs`.
9. Add a match arm for the new kind in `build()` in `src/channel/mod.rs`.
10. Add `warn_misplaced_fields()` entries in `src/config.rs` for any platform-specific config fields.
11. Add tests in `src/tests/channel/<name>_test.rs` and wire with `#[path = ...]` in the source file.
12. Update the channels table in `README.md` and add a field reference section in `docs/configuration.md`.

## Do Not

- Do not add a web UI, REST API, or HTTP server.
- Do not add database dependencies. State is in-memory.
- Do not manage CLI concurrency — backends own their own locking.
- Do not respond to unauthorized users — silent drop only.
- Do not hardcode tokens — env var interpolation or bust.
