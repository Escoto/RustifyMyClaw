$ErrorActionPreference = 'Stop'

$packageName = 'rustifymyclaw'
$toolsDir    = Split-Path -Parent $MyInvocation.MyCommand.Definition
$version     = '{{VERSION}}'
$url64       = "https://github.com/Escoto/RustifyMyClaw/releases/download/${version}/rustifymyclaw-${version}+x86_64-windows.zip"
$checksum64  = '{{CHECKSUM}}'

$packageArgs = @{
    PackageName    = $packageName
    Url64bit       = $url64
    UnzipLocation  = $toolsDir
    Checksum64     = $checksum64
    ChecksumType64 = 'sha256'
}

Install-ChocolateyZipPackage @packageArgs
