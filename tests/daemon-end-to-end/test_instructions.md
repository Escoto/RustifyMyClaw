# Daemon Test Instructions

Repeatable test plan for RustifyMyClaw running as a systemd daemon on Ubuntu.

**Workflow:** Run `test_setup_automation.sh` first, follow this document phase by phase, then run `test_cleanup.sh` when done.

---

## Prerequisites

- Ubuntu machine (or Debian-based) with systemd
- `sudo` access
- At least one CLI backend installed (`claude`, `codex`, or `gemini`)
- A messaging channel ready to test (Telegram is simplest)
- The repo cloned and on the `bug/daemon_nuances` branch

---

## Phase 0: Automated Setup

Run the setup script from the repo root on your Ubuntu machine:

```bash
chmod +x scripts/test_setup_automation.sh
sudo bash scripts/test_setup_automation.sh
```

The script will:

1. Install the `acl` package if missing.
2. Build the release binary and run `clippy` + `cargo test`.
3. Run `scripts/install.sh --system` (creates user, config dir, unit file).
4. Copy the freshly built binary over the one the installer downloaded.
5. Create a test workspace directory at `~/projects/test-workspace`.
6. Prompt you to fill in the config and env file.
7. Run `allow-path` on the test workspace.
8. Reload systemd.

When the script finishes, review its output and confirm all steps passed before continuing.

---

## Phase 1: Verify Installation

Run each check and tick it off:

```bash
# Binary
ls -la /usr/local/bin/rustifymyclaw

# Service user
id rustifymyclaw
# Expected: system UID, group=rustifymyclaw

# Group
getent group rustifymyclaw

# Config directory
ls -la /etc/rustifymyclaw/
# Expected: config.yaml (640 root:rustifymyclaw), env (640 root:rustifymyclaw)

stat -c '%a %U:%G' /etc/rustifymyclaw
# Expected: 750 root:rustifymyclaw

# Unit file
ls -la /etc/systemd/system/rustifymyclaw.service

# Unit loaded
systemctl status rustifymyclaw
# Expected: loaded (inactive is fine at this point)
```

| # | Check | Expected | Pass? |
|---|-------|----------|-------|
| 1 | Binary at `/usr/local/bin/rustifymyclaw` | File exists, 755 | |
| 2 | `id rustifymyclaw` | System UID, group rustifymyclaw, shell nologin | |
| 3 | `getent group rustifymyclaw` | Group exists | |
| 4 | Config dir perms | 750 root:rustifymyclaw | |
| 5 | `config.yaml` perms | 640 root:rustifymyclaw | |
| 6 | `env` perms | 640 root:rustifymyclaw | |
| 7 | Unit file exists | `/etc/systemd/system/rustifymyclaw.service` | |
| 8 | `systemctl status` shows loaded | Loaded (inactive OK) | |

---

## Phase 2: Verify Config Readability

```bash
# Daemon user can read config
sudo -u rustifymyclaw cat /etc/rustifymyclaw/config.yaml
# Should succeed (output the yaml)

# Daemon user can read env
sudo -u rustifymyclaw cat /etc/rustifymyclaw/env
# Should succeed
```

| # | Check | Expected | Pass? |
|---|-------|----------|-------|
| 1 | Daemon reads config.yaml | File contents printed | |
| 2 | Daemon reads env | File contents printed | |

---

## Phase 3: Config Path Resolution

```bash
# Without HOME, should resolve to /etc path
sudo -u rustifymyclaw env -u HOME /usr/local/bin/rustifymyclaw config path
# Expected: /etc/rustifymyclaw/config.yaml

# With RUSTIFYMYCLAW_CONFIG override
sudo -u rustifymyclaw env -u HOME RUSTIFYMYCLAW_CONFIG=/tmp/fake.yaml \
  /usr/local/bin/rustifymyclaw config path
# Expected: /tmp/fake.yaml
```

| # | Check | Expected | Pass? |
|---|-------|----------|-------|
| 1 | No HOME fallback | `/etc/rustifymyclaw/config.yaml` | |
| 2 | Env override | `/tmp/fake.yaml` | |

---

## Phase 4: Workspace Permissions

If you did NOT let the setup script run `allow-path` (or want to re-test):

```bash
sudo /usr/local/bin/rustifymyclaw config allow-path /home/$USER/projects/test-workspace
```

Verify:

```bash
# Workspace ACLs
getfacl /home/$USER/projects/test-workspace
# Expected: user:rustifymyclaw:rwx

# Parent traversal ACLs
getfacl /home/$USER
# Expected: user:rustifymyclaw:--x

getfacl /home/$USER/projects
# Expected: user:rustifymyclaw:--x

# Systemd override
cat /etc/systemd/system/rustifymyclaw.service.d/override.conf
# Expected: ReadWritePaths=/home/<user>/projects/test-workspace

# Idempotency — run again
sudo /usr/local/bin/rustifymyclaw config allow-path /home/$USER/projects/test-workspace
# Expected: "already in the systemd allowed paths"
```

| # | Check | Expected | Pass? |
|---|-------|----------|-------|
| 1 | Workspace ACL | `user:rustifymyclaw:rwx` | |
| 2 | Parent ACL (home) | `user:rustifymyclaw:--x` | |
| 3 | Parent ACL (projects) | `user:rustifymyclaw:--x` | |
| 4 | Override file content | `ReadWritePaths=<workspace>` under `[Service]` | |
| 5 | Idempotent re-run | "already in the systemd allowed paths" | |

---

## Phase 5: Start the Daemon

```bash
sudo systemctl daemon-reload
sudo systemctl start rustifymyclaw
```

Verify:

```bash
# Status
systemctl status rustifymyclaw
# Expected: active (running)

# Logs — check for clean startup
journalctl -u rustifymyclaw -n 50 --no-pager
# Look for: config loaded, workspace registered, channel started, NO errors

# Process user
ps aux | grep rustifymyclaw
# Expected: running as rustifymyclaw user
```

| # | Check | Expected | Pass? |
|---|-------|----------|-------|
| 1 | Status is `active (running)` | Yes | |
| 2 | Journal: config loaded | No env var or validation errors | |
| 3 | Journal: workspace registered | Workspace name appears | |
| 4 | Journal: channel started | Channel provider initialized | |
| 5 | Process user | `rustifymyclaw` | |
| 6 | No permission errors in journal | Clean | |

---

## Phase 6: Functional Tests (via messaging channel)

Open a tail on the journal in a separate terminal:

```bash
journalctl -u rustifymyclaw -f
```

Then send messages from your allowed user account:

| # | Send | Expected response | Journal check | Pass? |
|---|------|-------------------|---------------|-------|
| 1 | `hello, what directory are you working in?` | References the workspace path | Prompt dispatched, response sent | |
| 2 | Follow-up question about the previous answer | Coherent continuation (session active) | `-c` flag used (claude-cli) | |
| 3 | `/new` | Session reset confirmation | Session reset logged | |
| 4 | Question after `/new` | Fresh context (no memory of prior exchange) | No `-c` flag | |
| 5 | `/status` | Workspace and session info | | |
| 6 | `/help` | List of available commands | | |
| 7 | Ask for a very long code listing (>4000 chars) | Multiple chunked messages | Chunking logged | |
| 8 | **From a different, unauthorized account** | **No response at all** | `trace!` level log for unauthorized | |

---

## Phase 7: Config Hot-Reload

Keep the journal tail open.

### 7a: Rate limit reload

```bash
sudo bash -c 'cat >> /etc/rustifymyclaw/config.yaml << "EOF"

limits:
  max_requests: 2
  window_seconds: 60
EOF'
```

| # | Check | Expected | Pass? |
|---|-------|----------|-------|
| 1 | Journal within ~1s | "config change detected: rate limits (hot-reloaded)" | |
| 2 | Send 3 messages quickly | Third is rate-limited | |

### 7b: Invalid config

```bash
# Break the YAML
sudo bash -c 'echo "  :::invalid" >> /etc/rustifymyclaw/config.yaml'
```

| # | Check | Expected | Pass? |
|---|-------|----------|-------|
| 1 | Journal logs parse error | Yes, running config unchanged | |
| 2 | Send a message | Still works normally | |

```bash
# Fix it — remove the bad line
sudo sed -i '/:::invalid/d' /etc/rustifymyclaw/config.yaml
```

### 7c: Restart-required change

```bash
# Change workspace name (requires restart)
sudo sed -i 's/name: "test-workspace"/name: "renamed-workspace"/' /etc/rustifymyclaw/config.yaml
```

| # | Check | Expected | Pass? |
|---|-------|----------|-------|
| 1 | Journal shows warning | "workspaces added or removed - restart required" | |

```bash
# Revert
sudo sed -i 's/name: "renamed-workspace"/name: "test-workspace"/' /etc/rustifymyclaw/config.yaml
```

### 7d: Clean up rate limits

```bash
# Remove the limits block if you don't want it persisted
sudo sed -i '/^limits:/,/^[^ ]/{ /^limits:/d; /^  max_requests:/d; /^  window_seconds:/d; }' /etc/rustifymyclaw/config.yaml
```

---

## Phase 8: Restart & Stop Behavior

### 8a: Graceful restart

```bash
sudo systemctl restart rustifymyclaw
journalctl -u rustifymyclaw -n 20 --no-pager
```

| # | Check | Expected | Pass? |
|---|-------|----------|-------|
| 1 | Clean shutdown messages | Shutdown logged | |
| 2 | Clean startup messages | Config loaded, channels started | |
| 3 | No orphaned processes | `pgrep -c rustifymyclaw` returns 1 | |

### 8b: Graceful stop

```bash
sudo systemctl stop rustifymyclaw
journalctl -u rustifymyclaw -n 10 --no-pager
```

| # | Check | Expected | Pass? |
|---|-------|----------|-------|
| 1 | Shutdown log messages present | Yes | |
| 2 | Process gone | `pgrep rustifymyclaw` returns nothing | |

### 8c: Crash recovery (Restart=on-failure)

```bash
sudo systemctl start rustifymyclaw

# Kill it hard
sudo kill -9 $(pidof rustifymyclaw)

# Wait ~6 seconds (RestartSec=5)
sleep 6
systemctl status rustifymyclaw
```

| # | Check | Expected | Pass? |
|---|-------|----------|-------|
| 1 | Status after kill + wait | `active (running)` — systemd restarted it | |

```bash
sudo systemctl stop rustifymyclaw
```

---

## Phase 9: Edge Cases

For each test below, attempt a restart and check the journal. **Restore the config after each test.**

Keep a backup:

```bash
sudo cp /etc/rustifymyclaw/config.yaml /etc/rustifymyclaw/config.yaml.bak
sudo cp /etc/rustifymyclaw/env /etc/rustifymyclaw/env.bak
```

### 9a: Missing environment variable

```bash
sudo sed -i '/TELEGRAM_BOT_TOKEN/d' /etc/rustifymyclaw/env
sudo systemctl restart rustifymyclaw
journalctl -u rustifymyclaw -n 10 --no-pager
# Expected: error naming the missing variable
```

| Pass? | |
|-------|-|

Restore: `sudo cp /etc/rustifymyclaw/env.bak /etc/rustifymyclaw/env`

### 9b: Non-existent workspace directory

```bash
sudo sed -i 's|directory:.*|directory: "/nonexistent/path"|' /etc/rustifymyclaw/config.yaml
sudo systemctl restart rustifymyclaw
journalctl -u rustifymyclaw -n 10 --no-pager
# Expected: "directory does not exist"
```

| Pass? | |
|-------|-|

Restore: `sudo cp /etc/rustifymyclaw/config.yaml.bak /etc/rustifymyclaw/config.yaml`

### 9c: Empty allowed_users

```bash
sudo sed -i '/allowed_users:/,/^[^ ]/{/allowed_users:/!{/^  /d}}' /etc/rustifymyclaw/config.yaml
sudo sed -i 's/allowed_users:.*/allowed_users: []/' /etc/rustifymyclaw/config.yaml
sudo systemctl restart rustifymyclaw
journalctl -u rustifymyclaw -n 10 --no-pager
# Expected: "allowed_users must be non-empty"
```

| Pass? | |
|-------|-|

Restore: `sudo cp /etc/rustifymyclaw/config.yaml.bak /etc/rustifymyclaw/config.yaml`

### 9d: Unknown backend

```bash
sudo sed -i 's/backend:.*/backend: "unknown-cli"/' /etc/rustifymyclaw/config.yaml
sudo systemctl restart rustifymyclaw
journalctl -u rustifymyclaw -n 10 --no-pager
# Expected: "unknown backend"
```

| Pass? | |
|-------|-|

Restore: `sudo cp /etc/rustifymyclaw/config.yaml.bak /etc/rustifymyclaw/config.yaml`

### 9e: Config unreadable by daemon user

```bash
sudo chown root:root /etc/rustifymyclaw/config.yaml
sudo chmod 600 /etc/rustifymyclaw/config.yaml
sudo systemctl restart rustifymyclaw
journalctl -u rustifymyclaw -n 10 --no-pager
# Expected: "cannot read config file" or permission denied
```

| Pass? | |
|-------|-|

Restore:

```bash
sudo chown root:rustifymyclaw /etc/rustifymyclaw/config.yaml
sudo chmod 640 /etc/rustifymyclaw/config.yaml
```

### 9f: setfacl not installed

```bash
sudo apt remove -y acl
sudo /usr/local/bin/rustifymyclaw config allow-path /home/$USER/projects/test-workspace
# Expected: "setfacl is not installed"
sudo apt install -y acl
```

| Pass? | |
|-------|-|

---

## Phase 10: Boot Persistence

```bash
sudo systemctl enable rustifymyclaw
sudo reboot
```

After reboot:

```bash
systemctl status rustifymyclaw
# Expected: active (running)
```

Send a test message through your channel to confirm it's functional.

| # | Check | Expected | Pass? |
|---|-------|----------|-------|
| 1 | Service auto-started | `active (running)` | |
| 2 | Functional after reboot | Response received | |

---

## Done

When finished, run the cleanup script:

```bash
sudo bash scripts/test_cleanup.sh
```

See the cleanup script header for what it removes.
