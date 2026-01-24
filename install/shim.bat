@echo off
REM Covenant shim - auto-installs covenant if not found, then runs it.
REM Drop this into your project or CI to ensure covenant is available.
REM
REM Usage: shim.bat compile myfile.cov --output out.wasm
REM        shim.bat run myfile.cov

setlocal

set "INSTALL_DIR=%COVENANT_INSTALL%"
if "%INSTALL_DIR%"=="" set "INSTALL_DIR=%USERPROFILE%\.covenant"
set "COVENANT_BIN=%INSTALL_DIR%\bin\covenant.exe"

where covenant >nul 2>&1 && (
    covenant %*
    exit /b %errorlevel%
)

if exist "%COVENANT_BIN%" (
    "%COVENANT_BIN%" %*
    exit /b %errorlevel%
)

echo Covenant not found. Installing... >&2
powershell -NoProfile -ExecutionPolicy Bypass -Command "Invoke-Expression (Invoke-RestMethod 'https://raw.githubusercontent.com/Cyronius/covenant/master/install/install.ps1')"

if exist "%COVENANT_BIN%" (
    "%COVENANT_BIN%" %*
    exit /b %errorlevel%
) else (
    echo ERROR: Installation failed. >&2
    exit /b 1
)
