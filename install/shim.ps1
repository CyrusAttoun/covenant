#!/usr/bin/env pwsh
# Covenant shim - auto-installs covenant if not found, then runs it.
# Drop this into your project or CI to ensure covenant is available.
#
# Usage: .\shim.ps1 compile myfile.cov --output out.wasm
#        .\shim.ps1 run myfile.cov
#
# Environment:
#   COVENANT_VERSION   - Pin to specific version
#   COVENANT_INSTALL   - Override install directory

$ErrorActionPreference = "Stop"

$installDir = if ($env:COVENANT_INSTALL) { $env:COVENANT_INSTALL } else { Join-Path $HOME ".covenant" }
$covenantBin = Join-Path $installDir "bin\covenant.exe"

$existing = Get-Command covenant -ErrorAction SilentlyContinue
if ($existing) {
    & covenant @args
    exit $LASTEXITCODE
}

if (Test-Path $covenantBin) {
    & $covenantBin @args
    exit $LASTEXITCODE
}

Write-Host "Covenant not found. Installing..." -ForegroundColor Yellow
Invoke-Expression (Invoke-RestMethod "https://raw.githubusercontent.com/Cyronius/covenant/master/install/install.ps1")

if (Test-Path $covenantBin) {
    & $covenantBin @args
    exit $LASTEXITCODE
}
else {
    Write-Error "Installation failed."
    exit 1
}
