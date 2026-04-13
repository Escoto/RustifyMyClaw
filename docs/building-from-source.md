# Building from source

If there's no pre-built binary for your platform, you can build RustifyMyClaw yourself.

## Prerequisites

- **Rust toolchain** (stable, edition 2021) — install via [rustup](https://rustup.rs/)
- **Git**

```bash
# Verify your toolchain
rustc --version   # 1.70+ recommended
cargo --version
```

## Build

```bash
git clone https://github.com/Escoto/RustifyMyClaw.git
cd RustifyMyClaw
cargo build --release
```

The binary will be at `target/release/rustifymyclaw` (Linux/macOS) or `target\release\rustifymyclaw.exe` (Windows).

## Install

Copy the binary and example config to the default location:

**Linux / macOS:**

```bash
mkdir -p ~/.rustifymyclaw
cp target/release/rustifymyclaw ~/.rustifymyclaw/
cp examples/config.yaml ~/.rustifymyclaw/config.yaml

# Add to PATH (pick your shell's rc file)
echo 'export PATH="$HOME/.rustifymyclaw:$PATH"' >> ~/.bashrc
```

**Windows (PowerShell):**

```powershell
New-Item -ItemType Directory -Force -Path "$env:APPDATA\RustifyMyClaw"
Copy-Item target\release\rustifymyclaw.exe "$env:APPDATA\RustifyMyClaw\"
Copy-Item examples\config.yaml "$env:APPDATA\RustifyMyClaw\config.yaml"

# Add to user PATH
$p = [Environment]::GetEnvironmentVariable('Path', 'User')
[Environment]::SetEnvironmentVariable('Path', "$env:APPDATA\RustifyMyClaw;$p", 'User')
```

> **Windows users:** Pre-built binaries are also available via `choco install rustifymyclaw`. Building from source is only needed for development or unsupported platforms.

## Next steps

Edit `~/.rustifymyclaw/config.yaml` (or `%APPDATA%\RustifyMyClaw\config.yaml` on Windows) — see [configuration.md](configuration.md) for the full field reference.
