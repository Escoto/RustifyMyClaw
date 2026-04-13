$ErrorActionPreference = 'Stop'

$packageName = 'rustifymyclaw'

Uninstall-ChocolateyZipPackage -PackageName $packageName -ZipFileName "rustifymyclaw-*.zip"

$configDir = Join-Path $env:APPDATA 'RustifyMyClaw'
if (Test-Path $configDir) {
    Remove-Item -Recurse -Force $configDir
    Write-Host "Removed configuration directory: $configDir"
}
