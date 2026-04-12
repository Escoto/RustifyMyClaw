# Configuration Reference

## Config file resolution

RustifyMyClaw resolves the config path using this priority chain:

| Priority | Source | Example |
|----------|--------|---------|
| 1 | `-f` / `--config-file` CLI flag | `rustifymyclaw -f ./my-config.yaml` |
| 2 | `RUSTIFYMYCLAW_CONFIG` environment variable | `export RUSTIFYMYCLAW_CONFIG=~/projects/config.yaml` |
| 3 | `./config.yaml` in the current working directory | `cd my-project && rustifymyclaw` |
| 4 | Platform default | `~/.rustifymyclaw/config.yaml` (Unix) or `%APPDATA%\RustifyMyClaw\config.yaml` (Windows) |

The first match wins. Use `rustifymyclaw config path` to see which path would be used from your current directory.

## Full annotated example

```yaml
workspaces:
  - name: "my-project"                             # unique name, used by /use command
    directory: "/home/user/projects/my-project"    # must exist at startup
    backend: "claude-cli"                          # claude-cli | codex-cli | gemini-cli
    timeout_seconds: 300                           # optional — kill CLI process after N seconds
    channels:

      - kind: telegram
        bot_name: "@mybot"                         # optional — display/logging only
        token: "${TELEGRAM_BOT_TOKEN}"             # bot token from @BotFather
        allowed_users:
          - 123456789                              # numeric Telegram user ID
          - "@username"                            # or handle
        max_message_chars: 3500                    # optional — overrides global output.max_message_chars

      - kind: whatsapp
        token: "${WHATSAPP_API_TOKEN}"             # Meta Graph API token
        phone_number_id: "${WA_PHONE_NUMBER_ID}"  # Meta Business phone number ID (required)
        webhook_port: 8080                         # optional — inbound webhook port (default 8080)
        verify_token: "${WA_VERIFY_TOKEN}"         # optional — webhook verification token
        allowed_users:
          - "+15551234567"                         # phone number in E.164 format

      - kind: slack
        token: "${SLACK_BOT_TOKEN}"                # xoxb-* bot token
        app_token: "${SLACK_APP_TOKEN}"            # xapp-* Socket Mode token (required)
        use_threads: true                          # optional — reply in-thread (default false)
        max_message_chars: 3000                    # optional — Slack renders poorly above ~3000
        allowed_users:
          - "@dev_user"                            # Slack handle
          - "U01ABC123"                            # or raw Slack user ID

  - name: "data-pipeline"
    directory: "/home/user/projects/pipeline"
    backend: "codex-cli"
    channels:
      - kind: telegram
        token: "${PIPELINE_BOT_TOKEN}"
        allowed_users:
          - "@teammate"

output:
  max_message_chars: 4000                          # default max chars per message chunk
  file_upload_threshold_bytes: 51200               # 50 KB — responses larger than this are sent as a file
  chunk_strategy: "natural"                        # natural | fixed

# Optional — absent means no rate limiting
limits:
  max_requests: 10
  window_seconds: 60
```

## Field reference

### Top level

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `workspaces` | list | yes | At least one workspace required. |
| `output` | object | yes | Global output settings. |
| `limits` | object | no | Rate limiting policy. Absent = no limit. |

### `workspaces[]`

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | yes | — | Unique workspace name. Used by `/use` command. |
| `directory` | path | yes | — | Working directory for CLI invocations. Must exist at startup. |
| `backend` | string | yes | — | `claude-cli`, `codex-cli`, or `gemini-cli`. |
| `channels` | list | yes | — | At least one channel required. |
| `timeout_seconds` | integer | no | none | Kill the CLI process after this many seconds. Absent = no timeout. |

### `workspaces[].channels[]`

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `kind` | string | yes | — | `telegram`, `whatsapp`, or `slack`. |
| `token` | string | yes | — | Platform bot token. Supports `${ENV_VAR}`. |
| `allowed_users` | list | yes | — | Non-empty. Numeric IDs or string handles. |
| `bot_name` | string | no | — | Display name for logs only. |
| `max_message_chars` | integer | no | global | Override `output.max_message_chars` for this channel. |
| `file_upload_threshold_bytes` | integer | no | global | Override `output.file_upload_threshold_bytes` for this channel. |

**WhatsApp-only fields** (ignored with a warning on other channel kinds):

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `phone_number_id` | string | yes | — | Meta Business phone number ID. |
| `webhook_port` | integer | no | 8080 | Port for the inbound webhook HTTP server. |
| `verify_token` | string | no | — | Webhook verification token sent by Meta. |

**Slack-only fields** (ignored with a warning on other channel kinds):

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `app_token` | string | yes | — | `xapp-*` Socket Mode token. |
| `use_threads` | bool | no | false | Send responses as thread replies. |

### `output`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `max_message_chars` | integer | yes | Maximum characters per outbound message chunk. |
| `file_upload_threshold_bytes` | integer | yes | If total response exceeds this size, send as a file instead of chunking. |
| `chunk_strategy` | string | yes | `natural` (boundary-aware) or `fixed` (hard cut). |

### `limits`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `max_requests` | integer | yes | Maximum messages per user within the sliding window. |
| `window_seconds` | integer | yes | Sliding window width in seconds. |

## Environment variable interpolation

Any config value can reference an environment variable with `${VAR_NAME}`:

```yaml
token: "${TELEGRAM_BOT_TOKEN}"
```

The config loader replaces all `${...}` patterns at parse time using `std::env::var`. If a referenced variable is not set, RustifyMyClaw exits with an error at startup — no silent failures.

Never hardcode tokens directly in `config.yaml`. Use env var interpolation and keep your shell environment or a secrets manager as the source of truth.

## Per-channel output overrides

`max_message_chars` and `file_upload_threshold_bytes` can be set per channel to override the global `output` defaults. The merge rule:

- If the channel sets a value → use the channel value.
- If the channel omits a value → use the global `output` value.
- `chunk_strategy` is always global — there is no per-channel override.

This is handled by `effective_output_config()` in `src/config.rs` and applied at startup. Each channel stamps its resolved config onto every `InboundMessage` it produces.

## Backend-specific notes

**`claude-cli`** — Invokes `claude -p "<prompt>"`. When a session is active, adds the `-c` flag to continue the conversation. Session state is managed by `SessionStore`; `/new` resets it.

**`codex-cli`** — Invokes `codex -q "<prompt>"`. No session continuation flag — each invocation is independent.

**`gemini-cli`** — Invokes `gemini -p "<prompt>" -y`. No session continuation. The `-y` flag suppresses confirmation prompts.

## Validation

RustifyMyClaw validates the config at startup and exits with a descriptive error if any of the following are true:

- `workspaces` is empty.
- A workspace `name` is an empty string.
- A workspace `directory` does not exist on disk.
- A workspace `backend` is not one of `claude-cli`, `codex-cli`, `gemini-cli`.
- A workspace has no `channels`.
- A channel `kind` is not one of `telegram`, `whatsapp`, `slack`.
- A channel `allowed_users` list is empty.

If a platform-specific field appears on the wrong channel kind (e.g. `phone_number_id` on a Telegram channel), a warning is logged at startup and the field is ignored. The process does not exit.

## Config hot-reload

RustifyMyClaw watches `config.yaml` for changes using the `notify` crate. Changes are debounced (300ms). On a valid reload:

| Field(s) | Behavior |
|----------|----------|
| `limits` | Applied immediately — the in-memory rate limiter is updated. |
| `output` | Logged as changed. Requires restart to apply. |
| Workspaces added/removed | Logged as changed. Requires restart to apply. |
| Backend changed | Logged as changed. Requires restart to apply. |
| Channel tokens | Logged as changed. Requires restart to apply. |
| `allowed_users` | Logged as changed. Requires restart to apply. |

Invalid configs (YAML errors, missing env vars, validation failures) are logged and the running config remains in effect.
