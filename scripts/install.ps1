# RustifyMyClaw installer for Windows
# Usage:
#   irm https://raw.githubusercontent.com/Escoto/RustifyMyClaw/main/scripts/install.ps1 | iex
#   .\install.ps1 -Version v0.1.0
#   $env:VERSION = "v0.1.0"; irm ... | iex

#Requires -Version 5.1

param(
    [string]$Version = ""
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'  # Dramatically speeds up Invoke-WebRequest

$Repo = "Escoto/RustifyMyClaw"
$BinaryName = "rustifymyclaw.exe"
$InstallDir = Join-Path $env:APPDATA "RustifyMyClaw"
$ConfigFile = Join-Path $InstallDir "config.yaml"
$GitHubApi = "https://api.github.com/repos/$Repo"

# ---------------------------------------------------------------------------
# Output helpers
# ---------------------------------------------------------------------------
function Write-Info  { param([string]$Msg) Write-Host "[+] $Msg" -ForegroundColor Green }
function Write-Warn  { param([string]$Msg) Write-Host "[!] $Msg" -ForegroundColor Yellow }
function Write-Err   { param([string]$Msg) Write-Host "[-] $Msg" -ForegroundColor Red }

function Stop-WithError {
    param([string]$Msg)
    Write-Err $Msg
    exit 1
}

# ---------------------------------------------------------------------------
# Platform detection
# ---------------------------------------------------------------------------
function Get-Platform {
    $arch = $env:PROCESSOR_ARCHITECTURE
    switch ($arch) {
        "AMD64" { return "x86_64" }
        default { Stop-WithError "No pre-built binary for $arch Windows. You can build from source instead: https://github.com/$Repo/blob/main/docs/building-from-source.md" }
    }
}

# ---------------------------------------------------------------------------
# Version resolution
# ---------------------------------------------------------------------------
function Get-Version {
    # Priority: -Version param > $env:VERSION > GitHub API latest
    $ver = $Version
    if ([string]::IsNullOrEmpty($ver)) {
        $ver = $env:VERSION
    }

    if (-not [string]::IsNullOrEmpty($ver)) {
        if ($ver -notmatch '^v') { $ver = "v$ver" }
        return $ver
    }

    Write-Info "Fetching latest release..."
    try {
        $release = Invoke-RestMethod -Uri "$GitHubApi/releases/latest" -Headers @{ 'User-Agent' = 'RustifyMyClaw-Installer' }
        $ver = $release.tag_name
    }
    catch {
        $statusCode = $_.Exception.Response.StatusCode.value__
        if ($statusCode -eq 403) {
            Stop-WithError "GitHub API rate limit exceeded. Specify a version: `$env:VERSION = 'v0.1.0'"
        }
        Stop-WithError "Failed to fetch latest release: $_"
    }

    if ([string]::IsNullOrEmpty($ver)) {
        Stop-WithError "Could not determine latest version from GitHub API."
    }

    return $ver
}

# ---------------------------------------------------------------------------
# Download and verify
# ---------------------------------------------------------------------------
function Install-RustifyMyClaw {
    $arch = Get-Platform
    $resolvedVersion = Get-Version

    $artifact = "rustifymyclaw-${resolvedVersion}+${arch}-windows.zip"
    $baseUrl = "https://github.com/$Repo/releases/download/$resolvedVersion"
    $downloadUrl = "$baseUrl/$artifact"
    $checksumUrl = "$downloadUrl.sha256"

    $tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) "rustifymyclaw-install-$([System.Guid]::NewGuid().ToString('N').Substring(0,8))"

    try {
        New-Item -ItemType Directory -Force -Path $tmpDir | Out-Null
        $artifactPath = Join-Path $tmpDir $artifact
        $checksumPath = Join-Path $tmpDir "$artifact.sha256"

        # Download artifact
        Write-Info "Downloading $artifact..."
        try {
            Invoke-WebRequest -Uri $downloadUrl -OutFile $artifactPath -UseBasicParsing
        }
        catch {
            Stop-WithError "Download failed. Check that version $resolvedVersion exists at https://github.com/$Repo/releases"
        }

        # Download checksum
        Write-Info "Downloading checksum..."
        try {
            Invoke-WebRequest -Uri $checksumUrl -OutFile $checksumPath -UseBasicParsing
        }
        catch {
            Stop-WithError "Checksum download failed."
        }

        # Verify checksum
        Write-Info "Verifying SHA256 checksum..."
        $expectedHash = (Get-Content $checksumPath -Raw).Trim().Split(' ')[0].ToLower()
        $actualHash = (Get-FileHash -Path $artifactPath -Algorithm SHA256).Hash.ToLower()
        if ($expectedHash -ne $actualHash) {
            Stop-WithError "Checksum mismatch! Expected: $expectedHash, Got: $actualHash"
        }
        Write-Info "Checksum verified."

        # Create install directory
        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

        # Extract — the zip may contain nested paths from the release workflow,
        # so extract to temp and find the exe.
        $extractDir = Join-Path $tmpDir "extract"
        Expand-Archive -Path $artifactPath -DestinationPath $extractDir -Force

        $exeFile = Get-ChildItem -Path $extractDir -Recurse -Filter $BinaryName | Select-Object -First 1
        if (-not $exeFile) {
            Stop-WithError "Could not find $BinaryName in downloaded archive."
        }

        Copy-Item -Path $exeFile.FullName -Destination (Join-Path $InstallDir $BinaryName) -Force
        Write-Info "Binary installed at $(Join-Path $InstallDir $BinaryName)"

        # Config scaffold
        if (Test-Path $ConfigFile) {
            Write-Info "Existing config preserved at $ConfigFile"
        }
        else {
            $configUrl = "https://raw.githubusercontent.com/$Repo/main/examples/config.yaml"
            Write-Info "Downloading example config..."
            try {
                Invoke-WebRequest -Uri $configUrl -OutFile $ConfigFile -UseBasicParsing
            }
            catch {
                Stop-WithError "Failed to download example config from $configUrl"
            }
            Write-Info "Starter config created at $ConfigFile"
        }

        # PATH modification (user-level, no admin required)
        $currentUserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        if ([string]::IsNullOrEmpty($currentUserPath)) {
            [Environment]::SetEnvironmentVariable('Path', $InstallDir, 'User')
            Write-Info "PATH set to $InstallDir"
        }
        elseif ($currentUserPath -notlike "*$InstallDir*") {
            [Environment]::SetEnvironmentVariable('Path', "$InstallDir;$currentUserPath", 'User')
            Write-Info "PATH updated — added $InstallDir"
        }
        else {
            Write-Info "PATH already configured."
        }

        # Update current session
        if ($env:Path -notlike "*$InstallDir*") {
            $env:Path = "$InstallDir;$env:Path"
        }

        # Warn about shadowing
        $existing = Get-Command $BinaryName -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($existing -and $existing.Source -ne (Join-Path $InstallDir $BinaryName)) {
            Write-Warn "Another $BinaryName found at $($existing.Source) — it may shadow this install."
        }

        # Success
        Write-Host ""
        Write-Info "RustifyMyClaw $resolvedVersion installed successfully!"
        Write-Host ""
        Write-Host "  Binary:  $(Join-Path $InstallDir $BinaryName)"
        Write-Host "  Config:  $ConfigFile"
        Write-Host ""
        Write-Host "  Next steps:"
        Write-Host "    1. Edit $ConfigFile"
        Write-Host "       - Set your workspace directory"
        Write-Host "       - Configure your channel (Telegram / WhatsApp / Slack)"
        Write-Host "       - Set allowed_users"
        Write-Host "    2. Set required environment variables:"
        Write-Host '       $env:TELEGRAM_BOT_TOKEN = "your_token_here"'
        Write-Host "    3. Start the daemon:"
        Write-Host "       rustifymyclaw"
        Write-Host "    4. Restart your terminal for PATH changes to take effect"
        Write-Host ""
        Write-Host "  Full config reference:"
        Write-Host "  https://github.com/$Repo/blob/main/docs/configuration.md"
        Write-Host ""
    }
    finally {
        if (Test-Path $tmpDir) {
            Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
        }
    }
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "  RustifyMyClaw Installer" -ForegroundColor Cyan
Write-Host ""

Install-RustifyMyClaw
