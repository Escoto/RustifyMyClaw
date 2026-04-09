#!/usr/bin/env bash
set -euo pipefail

# RustifyMyClaw installer for Linux and macOS
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Escoto/RustifyMyClaw/main/scripts/install.sh | bash
#   curl -fsSL ... | bash -s -- v0.1.0
#   VERSION=v0.1.0 ./install.sh

REPO="Escoto/RustifyMyClaw"
BINARY_NAME="rustifymyclaw"
INSTALL_DIR="${HOME}/.rustifymyclaw"
CONFIG_FILE="${INSTALL_DIR}/config.yaml"
GITHUB_API="https://api.github.com/repos/${REPO}"

# ---------------------------------------------------------------------------
# Output helpers
# ---------------------------------------------------------------------------
USE_COLOR=false
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    USE_COLOR=true
fi

_c() {
    if $USE_COLOR; then printf "\033[%sm" "$1"; fi
}

info()  { printf "%s[+]%s %s\n" "$(_c '0;32')" "$(_c 0)" "$*"; }
warn()  { printf "%s[!]%s %s\n" "$(_c '0;33')" "$(_c 0)" "$*"; }
error() { printf "%s[-]%s %s\n" "$(_c '0;31')" "$(_c 0)" "$*" >&2; }
die()   { error "$@"; exit 1; }

# ---------------------------------------------------------------------------
# Prerequisite checks
# ---------------------------------------------------------------------------
DOWNLOAD_CMD=""
if command -v curl >/dev/null 2>&1; then
    DOWNLOAD_CMD="curl -fsSL"
elif command -v wget >/dev/null 2>&1; then
    DOWNLOAD_CMD="wget -qO-"
else
    die "curl or wget required. Install one and retry."
fi

download_file() {
    # download_file <url> <output_path>
    if [ "${DOWNLOAD_CMD%% *}" = "curl" ]; then
        curl -fsSL -o "$2" "$1"
    else
        wget -qO "$2" "$1"
    fi
}

command -v tar >/dev/null 2>&1 || die "tar is required but not found."

SHA256_CMD=""
if command -v sha256sum >/dev/null 2>&1; then
    SHA256_CMD="sha256sum"
elif command -v shasum >/dev/null 2>&1; then
    SHA256_CMD="shasum -a 256"
else
    die "sha256sum or shasum required for checksum verification."
fi

# ---------------------------------------------------------------------------
# Platform detection
# ---------------------------------------------------------------------------
detect_platform() {
    local os arch
    case "$(uname -s)" in
        Linux)  os="linux-gnu" ;;
        Darwin) os="apple-darwin" ;;
        *)      os="unknown" ;;
    esac

    case "$(uname -m)" in
        x86_64)         arch="x86_64" ;;
        aarch64|arm64)  arch="aarch64" ;;
        *)              arch="unknown" ;;
    esac

    # Supported release matrix — update when new targets are added
    local supported="x86_64-linux-gnu x86_64-apple-darwin aarch64-apple-darwin"
    local candidate="${arch}-${os}"

    for p in $supported; do
        if [ "$p" = "$candidate" ]; then
            PLATFORM="$candidate"
            return
        fi
    done

    die "No pre-built binary for $(uname -m) $(uname -s). You can build from source instead: https://github.com/${REPO}/blob/main/docs/building-from-source.md"
}

# ---------------------------------------------------------------------------
# Version resolution
# ---------------------------------------------------------------------------
resolve_version() {
    # Priority: $1 arg > $VERSION env var > GitHub API latest
    local version="${1:-${VERSION:-}}"

    if [ -n "$version" ]; then
        # Ensure v prefix
        case "$version" in
            v*) ;;
            *)  version="v${version}" ;;
        esac
    else
        info "Fetching latest release..."
        local api_response
        api_response=$($DOWNLOAD_CMD "${GITHUB_API}/releases/latest" 2>&1) || {
            if echo "$api_response" | grep -q "rate limit" 2>/dev/null; then
                die "GitHub API rate limit exceeded. Specify a version: VERSION=v0.1.0 $0"
            fi
            die "Failed to fetch latest release. Check network or specify VERSION=v0.1.0"
        }
        if command -v jq >/dev/null 2>&1; then
            version=$(echo "$api_response" | jq -r '.tag_name')
        else
            version=$(echo "$api_response" | grep '"tag_name"' | sed 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
        fi
        if [ -z "$version" ]; then
            die "Could not determine latest version from GitHub API."
        fi
    fi

    VERSION="$version"
}

# ---------------------------------------------------------------------------
# Download and verify
# ---------------------------------------------------------------------------
download_and_verify() {
    local artifact="rustifymyclaw-${VERSION}+${PLATFORM}.tar.gz"
    local base_url="https://github.com/${REPO}/releases/download/${VERSION}"
    local download_url="${base_url}/${artifact}"
    local checksum_url="${download_url}.sha256"

    TMP_DIR=$(mktemp -d)
    trap 'rm -rf "$TMP_DIR"' EXIT

    info "Downloading ${artifact}..."
    download_file "$download_url" "${TMP_DIR}/${artifact}" || \
        die "Download failed. Check that version ${VERSION} exists at https://github.com/${REPO}/releases"

    info "Downloading checksum..."
    download_file "$checksum_url" "${TMP_DIR}/${artifact}.sha256" || \
        die "Checksum download failed."

    info "Verifying SHA256 checksum..."
    local expected actual
    expected=$(awk '{print $1}' "${TMP_DIR}/${artifact}.sha256")
    actual=$($SHA256_CMD "${TMP_DIR}/${artifact}" | awk '{print $1}')
    if [ "$expected" != "$actual" ]; then
        die "Checksum mismatch! Expected: ${expected}, Got: ${actual}"
    fi
    info "Checksum verified."

    ARTIFACT_PATH="${TMP_DIR}/${artifact}"
}

# ---------------------------------------------------------------------------
# Install binary
# ---------------------------------------------------------------------------
install_binary() {
    mkdir -p "$INSTALL_DIR"

    info "Extracting to ${INSTALL_DIR}..."
    tar xzf "$ARTIFACT_PATH" -C "$INSTALL_DIR"
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

    # macOS: remove quarantine attribute
    if [ "$(uname -s)" = "Darwin" ]; then
        xattr -d com.apple.quarantine "${INSTALL_DIR}/${BINARY_NAME}" 2>/dev/null || true
    fi

    info "Binary installed at ${INSTALL_DIR}/${BINARY_NAME}"
}

# ---------------------------------------------------------------------------
# Config scaffold
# ---------------------------------------------------------------------------
write_config() {
    if [ -f "$CONFIG_FILE" ]; then
        info "Existing config preserved at ${CONFIG_FILE}"
        return
    fi

    local config_url="https://raw.githubusercontent.com/${REPO}/main/examples/config.yaml"
    info "Downloading example config..."
    download_file "$config_url" "$CONFIG_FILE" || \
        die "Failed to download example config from ${config_url}"

    chmod 600 "$CONFIG_FILE"
    info "Starter config created at ${CONFIG_FILE}"
}

# ---------------------------------------------------------------------------
# PATH modification
# ---------------------------------------------------------------------------
update_path() {
    local path_entry="export PATH=\"\$HOME/.rustifymyclaw:\$PATH\""
    local comment="# Added by RustifyMyClaw installer"
    local modified=()

    for rcfile in "${HOME}/.bashrc" "${HOME}/.zshrc" "${HOME}/.profile"; do
        # Only touch .zshrc if zsh exists, .profile always
        if [ "$rcfile" = "${HOME}/.zshrc" ] && ! command -v zsh >/dev/null 2>&1 && [ ! -f "$rcfile" ]; then
            continue
        fi
        # Skip .profile if .bashrc exists (most Linux distros source .bashrc from .profile)
        if [ "$rcfile" = "${HOME}/.profile" ] && [ -f "${HOME}/.bashrc" ]; then
            continue
        fi

        if [ -f "$rcfile" ] && grep -q 'rustifymyclaw' "$rcfile" 2>/dev/null; then
            continue
        fi

        # Create file if it doesn't exist (e.g. .zshrc on fresh macOS)
        printf '\n%s\n%s\n' "$comment" "$path_entry" >> "$rcfile"
        modified+=("$rcfile")
    done

    # Update current session
    export PATH="${INSTALL_DIR}:${PATH}"

    if [ ${#modified[@]} -gt 0 ]; then
        info "PATH updated in: ${modified[*]}"
    else
        info "PATH already configured."
    fi

    # Warn if binary shadows another installation
    local other
    other=$(command -v "$BINARY_NAME" 2>/dev/null || true)
    if [ -n "$other" ] && [ "$other" != "${INSTALL_DIR}/${BINARY_NAME}" ]; then
        warn "Another ${BINARY_NAME} found at ${other} — it may shadow this install."
    fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
main() {
    printf "\n  RustifyMyClaw Installer\n\n"

    detect_platform
    resolve_version "${1:-}"
    info "Installing RustifyMyClaw ${VERSION} for ${PLATFORM}"

    download_and_verify
    install_binary
    write_config
    update_path

    printf "\n"
    info "RustifyMyClaw ${VERSION} installed successfully!"
    printf "\n"
    printf "  Binary:  %s/%s\n" "$INSTALL_DIR" "$BINARY_NAME"
    printf "  Config:  %s\n" "$CONFIG_FILE"
    printf "\n"
    printf "  Next steps:\n"
    printf "    1. Edit %s\n" "$CONFIG_FILE"
    printf "       - Set your workspace directory\n"
    printf "       - Configure your channel (Telegram / WhatsApp / Slack)\n"
    printf "       - Set allowed_users\n"
    printf "    2. Export required environment variables:\n"
    printf "       export TELEGRAM_BOT_TOKEN=your_token_here\n"
    printf "    3. Start the daemon:\n"
    printf "       rustifymyclaw\n"
    printf "    4. Open a new terminal or run:  source ~/.bashrc\n"
    printf "\n"
    printf "  Full config reference:\n"
    printf "  https://github.com/%s/blob/main/docs/configuration.md\n\n" "$REPO"
}

main "$@"
