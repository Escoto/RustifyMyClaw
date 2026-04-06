# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-04-06

### Added

- Core pipeline: channel listener → router → executor → formatter → responder
- Telegram channel provider (teloxide, long-polling)
- WhatsApp channel provider (axum webhook server + Meta Graph API)
- Slack channel provider (Socket Mode via WebSocket)
- Claude Code CLI backend (`claude -p`, session continuation via `-c`)
- Codex CLI backend (`codex -q`)
- Gemini CLI backend (`gemini -p -y`)
- Per-channel `SecurityGate` with `allowed_users` allowlist
- Session tracking keyed by `ChatId` (channel-kind-aware, no cross-platform collisions)
- `/new` command — reset the current session
- `/use <workspace>` command — switch workspace at runtime
- `/status` command — show current workspace, backend, and session state
- `/help` command — list available commands
- Natural chunking strategy (code block → paragraph → line → sentence boundaries)
- Fixed chunking strategy (hard cut at `max_message_chars`)
- UTF-8 safe boundary detection via `char_boundary_floor()`
- File upload fallback for responses exceeding `file_upload_threshold_bytes`
- Per-channel output config overrides (`max_message_chars`, `file_upload_threshold_bytes`)
- YAML config with `${ENV_VAR}` interpolation at parse time
- Config validation with misplaced-field warnings for platform-specific keys
- Config hot-reload via `notify` file watcher (rate limits apply immediately; other changes require restart)
- Per-user sliding-window rate limiting
- CLI process timeout per workspace (`timeout_seconds`)
- Graceful shutdown on SIGTERM/Ctrl+C with 30-second drain timeout
- Windows support (`%APPDATA%\RustifyMyClaw\config.yaml` config path)
- 125 tests covering all modules

[0.1.0]: https://github.com/Escoto/RustifyMyClaw/releases/tag/v0.1.0
