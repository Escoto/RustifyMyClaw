#!/usr/bin/env bash
set -euo pipefail

# ---------------------------------------------------------------------------
# RustifyMyClaw — Daemon Test Cleanup
#
# Reverses everything done by test_setup_automation.sh and the manual test
# phases so the machine is back to a clean state.
#
# Run with: sudo bash scripts/test_cleanup.sh
#
# What this script removes:
#   - Stops and disables the systemd service
#   - Removes the unit file and override directory
#   - Removes the binary from /usr/local/bin
#   - Removes /etc/rustifymyclaw (config, env, backups)
#   - Removes POSIX ACLs from the test workspace and its parents
#   - Optionally removes the test workspace directory
#   - Removes the rustifymyclaw system user and group
#
# Idempotent — safe to run even if some artifacts are already gone.
# ---------------------------------------------------------------------------

REAL_USER="${SUDO_USER:-$USER}"
REAL_HOME=$(eval echo "~${REAL_USER}")
TEST_WORKSPACE="${REAL_HOME}/projects/test-workspace"

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

# ---------------------------------------------------------------------------
# Pre-flight
# ---------------------------------------------------------------------------
if [ "$(id -u)" -ne 0 ]; then
    die "Run with sudo:  sudo bash $0"
fi

printf "\n  RustifyMyClaw — Daemon Test Cleanup\n\n"
printf "  This will remove all daemon artifacts from this machine.\n"
printf "  (The repo and cargo build artifacts are NOT touched.)\n\n"
read -rp "  Continue? [y/N] " confirm
case "$confirm" in
    [yY]|[yY][eE][sS]) ;;
    *) echo "Aborted."; exit 0 ;;
esac

# ---------------------------------------------------------------------------
# Step 1: Stop and disable service
# ---------------------------------------------------------------------------
header "Step 1/7: Stop and disable service"

if systemctl is-active --quiet rustifymyclaw 2>/dev/null; then
    systemctl stop rustifymyclaw
    info "Service stopped."
else
    info "Service was not running."
fi

if systemctl is-enabled --quiet rustifymyclaw 2>/dev/null; then
    systemctl disable rustifymyclaw
    info "Service disabled."
else
    info "Service was not enabled."
fi

# ---------------------------------------------------------------------------
# Step 2: Remove unit file and overrides
# ---------------------------------------------------------------------------
header "Step 2/7: Remove systemd unit and overrides"

if [ -f /etc/systemd/system/rustifymyclaw.service ]; then
    rm -f /etc/systemd/system/rustifymyclaw.service
    info "Removed /etc/systemd/system/rustifymyclaw.service"
else
    info "Unit file already absent."
fi

if [ -d /etc/systemd/system/rustifymyclaw.service.d ]; then
    rm -rf /etc/systemd/system/rustifymyclaw.service.d
    info "Removed override directory."
else
    info "Override directory already absent."
fi

systemctl daemon-reload
info "systemd reloaded."

# ---------------------------------------------------------------------------
# Step 3: Remove binary
# ---------------------------------------------------------------------------
header "Step 3/7: Remove binary"

if [ -f /usr/local/bin/rustifymyclaw ]; then
    rm -f /usr/local/bin/rustifymyclaw
    info "Removed /usr/local/bin/rustifymyclaw"
else
    info "Binary already absent."
fi

# ---------------------------------------------------------------------------
# Step 4: Remove config directory
# ---------------------------------------------------------------------------
header "Step 4/7: Remove /etc/rustifymyclaw"

if [ -d /etc/rustifymyclaw ]; then
    rm -rf /etc/rustifymyclaw
    info "Removed /etc/rustifymyclaw"
else
    info "Config directory already absent."
fi

# ---------------------------------------------------------------------------
# Step 5: Remove ACLs
# ---------------------------------------------------------------------------
header "Step 5/7: Remove POSIX ACLs"

if command -v setfacl >/dev/null 2>&1; then
    # Workspace ACLs
    if [ -d "$TEST_WORKSPACE" ]; then
        setfacl -R -b "$TEST_WORKSPACE" 2>/dev/null || true
        info "Cleared ACLs on ${TEST_WORKSPACE}"
    fi

    # Parent traversal ACLs — only remove the rustifymyclaw user entry, not all ACLs
    for dir in "${REAL_HOME}" "${REAL_HOME}/projects"; do
        if [ -d "$dir" ]; then
            setfacl -x u:rustifymyclaw "$dir" 2>/dev/null || true
            info "Removed rustifymyclaw ACL from ${dir}"
        fi
    done
else
    warn "setfacl not found — skipping ACL cleanup. Install acl and re-run, or clean manually."
fi

# ---------------------------------------------------------------------------
# Step 6: Optionally remove test workspace
# ---------------------------------------------------------------------------
header "Step 6/7: Test workspace"

if [ -d "$TEST_WORKSPACE" ]; then
    printf "  Remove %s? [y/N] " "$TEST_WORKSPACE"
    read -r remove_ws
    case "$remove_ws" in
        [yY]|[yY][eE][sS])
            rm -rf "$TEST_WORKSPACE"
            info "Removed ${TEST_WORKSPACE}"
            ;;
        *)
            info "Kept ${TEST_WORKSPACE}"
            ;;
    esac
else
    info "Test workspace already absent."
fi

# ---------------------------------------------------------------------------
# Step 7: Remove system user and group
# ---------------------------------------------------------------------------
header "Step 7/7: Remove system user and group"

if id -u rustifymyclaw >/dev/null 2>&1; then
    userdel rustifymyclaw 2>/dev/null || true
    info "Removed user rustifymyclaw."
else
    info "User rustifymyclaw already absent."
fi

if getent group rustifymyclaw >/dev/null 2>&1; then
    groupdel rustifymyclaw 2>/dev/null || true
    info "Removed group rustifymyclaw."
else
    info "Group rustifymyclaw already absent."
fi

# ---------------------------------------------------------------------------
# Done
# ---------------------------------------------------------------------------
printf "\n"
success "Cleanup complete. Machine is back to pre-test state."
printf "\n  The repo and build artifacts were not touched.\n"
printf "  To re-test, run:  sudo bash scripts/test_setup_automation.sh\n\n"
