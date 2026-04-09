# RustifyMyClaw

[![CI](https://github.com/Escoto/RustifyMyClaw/actions/workflows/ci.yml/badge.svg)](https://github.com/Escoto/RustifyMyClaw/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)

Lightweight Rust daemon that bridges messaging platforms to local AI CLI tools.

```
┌──────────────┐
│  Telegram    │──┐
├──────────────┤  │      ┌─────────────────────────────────────────┐      ┌──────────────┐
│  WhatsApp    │──┼────▶│                RustifyMyClaw            │────▶ │  claude      │
├──────────────┤  │      │  Security → Router → Executor → Format  │◀─── │  codex       │
│  Slack       │──┘      └─────────────────────────────────────────┘      │  gemini      │
└──────────────┘                                                          └──────────────┘
```

## Why this exists

AI CLI tools are powerful but terminal-bound. RustifyMyClaw lets you use them from Telegram, WhatsApp, or Slack without a web server, a database, or any infrastructure beyond the daemon itself. One config file, one binary, running in the background.

## Quickstart

### Build from source

[docs/building-from-source](docs/building-from-source.md)

### Install

**Linux / macOS:**

```bash
curl -fsSL https://raw.githubusercontent.com/Escoto/RustifyMyClaw/main/scripts/install.sh | bash
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/Escoto/RustifyMyClaw/main/scripts/install.ps1 | iex
```

Install a specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/Escoto/RustifyMyClaw/main/scripts/install.sh | bash -s -- v0.1.0
```

The installer downloads the binary, verifies its SHA256 checksum, creates a starter `config.yaml`, and adds it to your PATH. 

> [!IMPORTANT]
> You must update the starter **config.yaml**:
> * %APPDATA%\RustifyMyClaw
> * ~/.rustifymyclaw

## Configuration

Minimal `config.yaml` to get started:

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
  max_message_chars: 4000
  file_upload_threshold_bytes: 51200
  chunk_strategy: "natural"
```

Tokens are never hardcoded — use `${ENV_VAR}` interpolation. Full reference: [docs/configuration](docs/configuration.md)

## Backends

| Backend | Binary | Status |
|---------|--------|--------|
| Claude Code | `claude` | Stable |
| Codex | `codex` | Stable |
| Gemini CLI | `gemini` | Stable |

## Channels

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

## Architecture

See [docs/architecture.md](docs/architecture.md) for the full system design, data flow walkthrough, and extension points.

## Contributing

Contributions welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under [Apache-2.0](LICENSE).
