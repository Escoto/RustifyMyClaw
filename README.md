# RustifyMyClaw

[![CI](https://github.com/Escoto/RustifyMyClaw/actions/workflows/ci.yml/badge.svg)](https://github.com/Escoto/RustifyMyClaw/actions/workflows/ci.yml)
[![GitHub release](https://img.shields.io/github/v/release/Escoto/RustifyMyClaw)](https://github.com/Escoto/RustifyMyClaw/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/Escoto/RustifyMyClaw/total)](https://github.com/Escoto/RustifyMyClaw/releases)
[![Chocolatey](https://img.shields.io/chocolatey/v/rustifymyclaw)](https://community.chocolatey.org/packages/rustifymyclaw)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Made with Rust](https://img.shields.io/badge/Made%20with-Rust-orange?logo=rust)](https://www.rust-lang.org)

**A secure, self-hosted proxy that connects Telegram, WhatsApp, and Slack to your local AI CLI tools — Claude Code, Codex, and Gemini CLI.**

No database. No cloud. No accounts. A single Rust binary and one YAML config file, running entirely on your machine.

```
  Telegram ─┐                                              ┌─ Claude Code
  WhatsApp ─┼──► SecurityGate ► Router ► Executor (local) ─┼─ Codex
  Slack    ─┘       │                                      └─ Gemini CLI
                    │
               Formatter ► back to chat
```

> Your prompt goes in. The model's response comes out. Your CLI tool owns its configuration and governs the agent. **RustifyMyClaw never touches your commands or runs commands on your behalf**. Nothing is stored, modified, or logged.

## Why RustifyMyClaw

### The problem

AI CLI tools like Claude Code, Codex, and Gemini CLI are powerful — but they're locked to your terminal. You can't reach them while away from your desk. Existing bridges require cloud hosting, databases, and trusting a third party with your prompts and running wild on your pc.

### The solution

RustifyMyClaw is a secure local proxy daemon written in Rust. It runs on your machine, accepts messages from your messaging platform, passes them **unmodified** to the CLI tool you configured, and returns the response. One binary. One YAML file. Nothing else.

### Your prompts stay yours

- **No database.** No message logging. All state is volatile in-memory — a boolean and a timestamp per session. Restart the daemon and it's a clean slate.
- **No prompt modification.** Your message is passed directly to the CLI as-is. No inspection, no rewriting, no rerouting.
- **No model override.** The backend you configured is the backend that runs. Period.
- **No agent configuration.** Your CLI tool's own their own settings — project instructions, tool permissions, safety rules — remain the sole authority over what the agent can and cannot do. RustifyMyClaw doesn't add, remove, or override any of it.
- **No cloud. No telemetry. No phone-home.** The daemon listens to your messaging platform's API to receive and send messages. Everything else happens locally. Self-hosted means self-hosted.

## How it works

1. A message arrives on your configured channel (Telegram, WhatsApp, or Slack).
2. **SecurityGate** checks the sender against your per-channel allowlist. Unauthorized messages are silently dropped — no error response, no acknowledgment.
3. **Router** parses the message. Commands (`/new`, `/use`, `/status`, `/help`) are handled internally. Everything else is a prompt.
4. **Executor** spawns your configured CLI tool as a local process in your project directory. Your prompt is passed through unmodified.
5. **Formatter** chunks the CLI output intelligently — respecting code block boundaries, paragraph breaks, and UTF-8 character boundaries — then sends it back to the originating chat.

The daemon never modifies your prompt, never overrides your model, and never persists any data. It is a pass-through proxy. Your CLI tool's own configuration — project instructions, tool permissions, safety rules — governs what the agent can do, not RustifyMyClaw.

## Features

**Security & Privacy**
- Zero-trust gateway — per-channel user allowlists with platform-native identity validation
- Unauthorized messages silently dropped (no information leakage to attackers)
- No database, no logs, no persistent state of any kind
- Environment variable interpolation for all secrets — zero hardcoded tokens

**Intelligent Output**
- Natural chunking that respects code block boundaries (fenced blocks never split mid-block)
- UTF-8 safe splitting — never panics on emoji or multibyte characters
- Automatic file upload when responses exceed a configurable threshold

**Operations**
- Per-user sliding-window rate limiting (configurable, optional)
- Config hot-reload — rate limit changes apply immediately without restart
- Graceful shutdown with 30-second in-flight message drain
- Process timeout enforcement per workspace (prevents runaway CLI sessions)
- Structured logging via `tracing` with configurable levels

**Quality**
- 130+ tests, zero clippy warnings, `cargo fmt` enforced
- Trait-based extensibility — add backends or channels by implementing one trait
- Single binary, one YAML config, cross-platform (Linux, macOS, Windows)

## Quickstart

### 1. Install

**Linux / macOS:**

```bash
curl -fsSL https://raw.githubusercontent.com/Escoto/RustifyMyClaw/main/scripts/install.sh | bash
```

**Windows (Chocolatey):**

```powershell
choco install rustifymyclaw
```

**Windows (PowerShell script):**

```powershell
irm https://raw.githubusercontent.com/Escoto/RustifyMyClaw/main/scripts/install.ps1 | iex
```

The installer downloads the binary, verifies its SHA256 checksum, creates a starter config, and adds it to your PATH. To install a specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/Escoto/RustifyMyClaw/main/scripts/install.sh | bash -s -- v0.1.0
```

Or [build from source](docs/building-from-source.md).

### 2. Configure

Generate a starter config with `config init`:

```bash
rustifymyclaw config init                # writes to default platform location
rustifymyclaw config init -d .           # writes config.yaml in current directory
rustifymyclaw config init -f my.yaml     # writes to a specific file path
```

Default locations:
- **Linux / macOS:** `~/.rustifymyclaw/config.yaml`
- **Windows:** `%APPDATA%\RustifyMyClaw\config.yaml`

RustifyMyClaw auto-discovers `config.yaml` in the current directory, so `config init -d .` followed by `rustifymyclaw` just works. See [docs/configuration.md](docs/configuration.md) for the full resolution chain.

Minimal example:

```yaml
workspaces:
  - name: "my-project"
    directory: "/home/user/projects/my-project"
    backend: "claude-cli"
    channels:
      - kind: telegram
        token: "${TELEGRAM_BOT_TOKEN}"
        allowed_users:
          - "@your_handle"

output:
  max_message_chars: 600
  file_upload_threshold_bytes: 51200
  chunk_strategy: "natural"
```

Tokens are never hardcoded — use `${ENV_VAR}` interpolation. Full reference: [docs/configuration.md](docs/configuration.md)

### 3. Run

Use default config location OR current directory when `config.yaml` is present:

```bash
rustifymyclaw
```

Or with a custom config path:

```bash
rustifymyclaw -f /path/to/config.yaml
```

Validate your config without starting the daemon:

```bash
rustifymyclaw --validate
```

## Backends

RustifyMyClaw proxies to whichever AI CLI tool you have installed locally. Adding a new backend is one file and one trait implementation — see [How to Add a New Backend](CLAUDE.md).

| Backend | Binary | Status |
|---------|--------|--------|
| Claude Code | `claude` | Stable |
| Codex | `codex` | Stable |
| Gemini CLI | `gemini` | Stable |

## Channels

Each channel connects using the platform's native protocol. No webhooks required for Telegram or Slack.

| Channel | Mode | Status |
|---------|------|--------|
| Telegram | Long-polling | Stable |
| WhatsApp | Webhook | Stable |
| Slack | Socket Mode | Stable |

## Chat commands

| Command | Description |
|---------|-------------|
| `/new` | Reset the current session |
| `/use <workspace>` | Switch to a different workspace |
| `/status` | Show current workspace, backend, and session state |
| `/help` | List available commands |

## Documentation

- [Architecture](docs/architecture.md) — system design, data flow, and extension points
- [Configuration](docs/configuration.md) — full field reference and examples
- [Building from Source](docs/building-from-source.md) — build instructions and requirements

## Contributing

Contributions welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under [Apache-2.0](LICENSE).
