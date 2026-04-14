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

AI CLI tools are powerful but locked to your terminal. Existing bridges require cloud hosting, databases, and trusting a third party with your prompts.

RustifyMyClaw runs locally. Messages in -> directly to your Agent, responses out -> directly back to you - zero tinkering with your requests. One binary, one YAML file, and your Agent's own config.

### Your prompts stay yours

- **No database.** The only state is `is_active: bool` — whether the conversation has an ongoing session (so the backend knows to pass `--continue` or not). Restart = clean slate.
- **No prompt modification.** Messages pass to the CLI as-is.
- **No agent override.** Your CLI tool's own config governs what the agent can do. RustifyMyClaw doesn't touch it.
- **No cloud. No telemetry.** Talks to your messaging platform's API. Everything else is local.

## How it works

1. Message arrives on your channel (Telegram, WhatsApp, or Slack).
2. **SecurityGate** checks the sender against your allowlist. Unauthorized = silent drop.
3. **Router** parses commands (`/new`, `/use`, `/status`, `/help`). Everything else is a prompt.
4. **Executor** spawns your CLI tool locally. Prompt passed through unmodified.
5. **Formatter** chunks the output respecting code blocks and UTF-8 boundaries, sends it back.

## Features

- Code-block-aware output chunking — fenced blocks never split mid-block, UTF-8 safe
- Auto file upload when responses exceed a configurable threshold
- Per-user rate limiting with config hot-reload (no restart needed)
- Graceful shutdown with 30s in-flight message drain
- Per-workspace process timeout to prevent runaway sessions
- Env var interpolation for all secrets — zero hardcoded tokens
- 140+ tests, zero clippy warnings, trait-based extensibility
- Single binary, cross-platform (Linux, macOS, Windows)

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
