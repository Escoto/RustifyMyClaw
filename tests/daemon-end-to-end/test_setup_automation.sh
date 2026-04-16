#!/usr/bin/env bash
set -euo pipefail

# ---------------------------------------------------------------------------
# RustifyMyClaw — Daemon Test Setup
#
# Automates the repeatable setup for daemon testing on Ubuntu/Debian.
# Run from the repo root with: sudo bash scripts/test_setup_automation.sh
#
# What this script does:
#   1. Installs the acl package if missing
#   2. Builds the release binary and runs clippy + tests
#   3. Runs scripts/install.sh --system
#   4. Copies the freshly built binary over the downloaded one
#   5. Creates a test workspace directory
#   6. Pauses for you to edit config.yaml and env
#   7. Runs allow-path on the test workspace
#   8. Reloads systemd
#
# Idempotent — safe to run multiple times.
# ---------------------------------------------------------------------------

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
REAL_USER="${SUDO_USER:-$USER}"
REAL_HOME=$(eval echo "~${REAL_USER}")
TEST_WORKSPACE="${REAL_HOME}/projects/test-workspace"
CONFIG_FILE="/etc/rustifymyclaw/config.yaml"
ENV_FILE="/etc/rustifymyclaw/env"
BINARY="target/release/rustifymyclaw"

# ---------------------------------------------------------------------------
# Output helpers
# ---------------------------------------------------------------------------
USE_COLOR=false
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    USE_COLOR=true
fi

_c() { if $USE_COLOR; then printf "\033[%sm" "$1"; fi; }

info()    { printf "%s[+]%s %s\n" "$(_c '0;32')" "$(_c 0)" "$*"; }
warn()    { printf "%s[!]%s %s\n" "$(_c '0;33')" "$(_c 0)" "$*"; }
error()   { printf "%s[-]%s %s\n" "$(_c '0;31')" "$(_c 0)" "$*" >&2; }
die()     { error "$@"; exit 1; }
header()  { printf "\n%s=== %s ===%s\n\n" "$(_c '1;36')" "$*" "$(_c 0)"; }
success() { printf "\n%s[OK]%s %s\n" "$(_c '1;32')" "$(_c 0)" "$*"; }

pause_for_user() {
    printf "\n%s[?]%s %s\n" "$(_c '1;33')" "$(_c 0)" "$*"
    read -rp "    Press Enter when ready (or Ctrl-C to abort)..."
}

# ---------------------------------------------------------------------------
# Pre-flight
# ---------------------------------------------------------------------------
if [ "$(id -u)" -ne 0 ]; then
    die "Run with sudo:  sudo bash $0"
fi

if [ ! -f "${REPO_ROOT}/Cargo.toml" ]; then
    die "Run from the repo root. Could not find Cargo.toml in ${REPO_ROOT}"
fi

cd "$REPO_ROOT"

# ---------------------------------------------------------------------------
# Step 1: Install acl package
# ---------------------------------------------------------------------------
header "Step 1/8: Install acl package"

if command -v setfacl >/dev/null 2>&1; then
    info "acl is already installed."
else
    info "Installing acl..."
    apt-get update -qq && apt-get install -y -qq acl
    info "acl installed."
fi

# ---------------------------------------------------------------------------
# Step 2: Build and validate
# ---------------------------------------------------------------------------
header "Step 2/8: Build release binary + clippy + tests"

info "Running cargo fmt --check..."
su - "$REAL_USER" -c "cd ${REPO_ROOT} && cargo fmt --check" || \
    die "cargo fmt --check failed. Run 'cargo fmt' first."

info "Building release binary..."
su - "$REAL_USER" -c "cd ${REPO_ROOT} && cargo build --release"

info "Running clippy..."
su - "$REAL_USER" -c "cd ${REPO_ROOT} && cargo clippy -- -D warnings"

info "Running tests..."
su - "$REAL_USER" -c "cd ${REPO_ROOT} && cargo test"

success "Build and validation passed."

# ---------------------------------------------------------------------------
# Step 3: System install
# ---------------------------------------------------------------------------
header "Step 3/8: Run install.sh --system"

bash "${REPO_ROOT}/scripts/install.sh" --system
success "System install completed."

# ---------------------------------------------------------------------------
# Step 4: Replace binary with freshly built one
# ---------------------------------------------------------------------------
header "Step 4/8: Copy local build to /usr/local/bin/"

install -m 755 "${REPO_ROOT}/${BINARY}" /usr/local/bin/rustifymyclaw
info "Installed $(rustifymyclaw --version 2>/dev/null || echo 'local build') to /usr/local/bin/"
success "Local binary deployed."

# ---------------------------------------------------------------------------
# Step 5: Create test workspace
# ---------------------------------------------------------------------------
header "Step 5/8: Create test workspace"

if [ -d "$TEST_WORKSPACE" ]; then
    info "Test workspace already exists at ${TEST_WORKSPACE}"
else
    su - "$REAL_USER" -c "mkdir -p ${TEST_WORKSPACE}"
    info "Created ${TEST_WORKSPACE}"
fi

success "Workspace ready at ${TEST_WORKSPACE}"

# ---------------------------------------------------------------------------
# Step 6: Config and env — interactive
# ---------------------------------------------------------------------------
header "Step 6/8: Configure config.yaml and env"

if [ -f "$CONFIG_FILE" ]; then
    info "Current config.yaml:"
    echo "---"
    cat "$CONFIG_FILE"
    echo "---"
fi

cat <<GUIDANCE

You need to edit two files before continuing:

  1. ${CONFIG_FILE}
     - Set directory to: ${TEST_WORKSPACE}
     - Set backend to whichever CLI you have (claude-cli, codex-cli, gemini-cli)
     - Set the channel kind and allowed_users for your test account

     Minimal example:

       workspaces:
         - name: "test-workspace"
           directory: "${TEST_WORKSPACE}"
           backend: "claude-cli"
           timeout_seconds: 120
           channels:
             - kind: telegram
               token: "\${TELEGRAM_BOT_TOKEN}"
               allowed_users:
                 - 123456789

       output:
         max_message_chars: 4000
         file_upload_threshold_bytes: 51200
         chunk_strategy: "natural"

  2. ${ENV_FILE}
     - Uncomment and fill in the token(s) for your channel
     - Example: TELEGRAM_BOT_TOKEN=<your-token>

GUIDANCE

pause_for_user "Edit ${CONFIG_FILE} and ${ENV_FILE} now (in another terminal), then come back here."

# Verify file permissions: run "cat" as the rustifymyclaw *user*
info "Verifying config is readable by daemon user..."
if sudo -u rustifymyclaw cat "$CONFIG_FILE" >/dev/null 2>&1; then
    info "Daemon user can read config.yaml"
else
    warn "Daemon user CANNOT read config.yaml — check ownership (should be root:rustifymyclaw 640)"
fi

if sudo -u rustifymyclaw cat "$ENV_FILE" >/dev/null 2>&1; then
    info "Daemon user can read env"
else
    warn "Daemon user CANNOT read env — check ownership (should be root:rustifymyclaw 640)"
fi

success "Configuration step done."

# ---------------------------------------------------------------------------
# Step 7: allow-path
# ---------------------------------------------------------------------------
header "Step 7/8: Grant daemon access to workspace"

/usr/local/bin/rustifymyclaw config allow-path "$TEST_WORKSPACE"

info "Verifying daemon user has read/write access to workspace..."
TEST_FILE="${TEST_WORKSPACE}/.rustifymyclaw-acl-test"
if sudo -u rustifymyclaw touch "$TEST_FILE" 2>/dev/null; then
    if sudo -u rustifymyclaw rm "$TEST_FILE" 2>/dev/null; then
        info "Daemon user can read and write in ${TEST_WORKSPACE}"
    else
        warn "Daemon user can create but NOT delete files in ${TEST_WORKSPACE}"
    fi
else
    warn "Daemon user CANNOT write to ${TEST_WORKSPACE} — ACLs may not have applied correctly"
fi

success "allow-path completed for ${TEST_WORKSPACE}"

# ---------------------------------------------------------------------------
# Step 8: Reload systemd
# ---------------------------------------------------------------------------
header "Step 8/8: Reload systemd"

systemctl daemon-reload
info "systemd reloaded."

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
printf "\n"
success "Setup complete!"
cat <<SUMMARY

  Next steps — follow docs/test_instructions.md starting from Phase 1.

  Quick-start:
    sudo systemctl start rustifymyclaw
    journalctl -u rustifymyclaw -f

  Config backups (for Phase 9 edge-case testing):
    sudo cp ${CONFIG_FILE} ${CONFIG_FILE}.bak
    sudo cp ${ENV_FILE} ${ENV_FILE}.bak

SUMMARY
