#!/usr/bin/env pwsh
# Covenant installer for Windows
# Usage: irm https://raw.githubusercontent.com/Cyronius/covenant/master/install/install.ps1 | iex
#
# Environment variables:
#   COVENANT_INSTALL   - Installation directory (default: $HOME\.covenant)
#   COVENANT_VERSION   - Specific version to install (default: latest)

$ErrorActionPreference = "Stop"

$Repo = "Cyronius/covenant"

function Install-Covenant {
    $arch = Get-Architecture
    $installDir = if ($env:COVENANT_INSTALL) { $env:COVENANT_INSTALL } else { Join-Path $HOME ".covenant" }
    $binDir = Join-Path $installDir "bin"
    $version = Get-CovenantVersion

    $archiveName = "covenant-${version}-windows-${arch}.zip"
    $url = "https://github.com/${Repo}/releases/download/v${version}/${archiveName}"

    Write-Host "Installing " -NoNewline
    Write-Host "Covenant v${version} (windows/${arch})" -ForegroundColor Green

    $tmpDir = New-TemporaryFile | ForEach-Object { Remove-Item $_; New-Item -ItemType Directory -Path $_ }
    $zipPath = Join-Path $tmpDir $archiveName

    try {
        Invoke-WebRequest -Uri $url -OutFile $zipPath -UseBasicParsing

        New-Item -ItemType Directory -Path $binDir -Force | Out-Null
        Expand-Archive -Path $zipPath -DestinationPath $binDir -Force

        $covenantExe = Join-Path $binDir "covenant.exe"
        if (-not (Test-Path $covenantExe)) {
            throw "Installation failed: covenant.exe not found after extraction."
        }

        Add-ToPath $binDir

        Write-Host ""
        Write-Host "Installed " -NoNewline
        Write-Host "Covenant to $covenantExe" -ForegroundColor Green
        Write-Host ""
        Write-Host "  Run 'covenant --help' to get started."
        Write-Host ""
        Write-Host "  To uninstall: Remove-Item -Recurse $installDir"
    }
    finally {
        Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
    }
}

function Get-Architecture {
    $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    switch ($arch) {
        "X64"   { return "x86_64" }
        "Arm64" { return "aarch64" }
        default { throw "Unsupported architecture: $arch" }
    }
}

function Get-CovenantVersion {
    if ($env:COVENANT_VERSION) {
        return $env:COVENANT_VERSION
    }

    try {
        $response = Invoke-RestMethod "https://api.github.com/repos/${Repo}/releases/latest"
        $tag = $response.tag_name
        return $tag -replace '^v', ''
    }
    catch {
        throw "Failed to determine latest version. Set COVENANT_VERSION manually."
    }
}

function Add-ToPath {
    param([string]$Dir)

    $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($currentPath -like "*$Dir*") {
        return
    }

    [Environment]::SetEnvironmentVariable("Path", "$Dir;$currentPath", "User")
    $env:Path = "$Dir;$env:Path"

    Write-Host "  Added $Dir to User PATH." -ForegroundColor Cyan
    Write-Host "  Restart your terminal for PATH changes to take effect."
}

Install-Covenant
