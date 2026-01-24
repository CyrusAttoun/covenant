#!/bin/sh
# Covenant shim - auto-installs covenant if not found, then runs it.
# Drop this into your project or CI to ensure covenant is available.
#
# Usage: ./shim.sh compile myfile.cov --output out.wasm
#        ./shim.sh run myfile.cov
#
# Environment:
#   COVENANT_VERSION   - Pin to specific version
#   COVENANT_INSTALL   - Override install directory

set -e

INSTALL_DIR="${COVENANT_INSTALL:-$HOME/.covenant}"
COVENANT_BIN="$INSTALL_DIR/bin/covenant"

if command -v covenant >/dev/null 2>&1; then
    exec covenant "$@"
elif [ -x "$COVENANT_BIN" ]; then
    exec "$COVENANT_BIN" "$@"
else
    echo "Covenant not found. Installing..." >&2
    curl -fsSL https://raw.githubusercontent.com/Cyronius/covenant/master/install/install.sh | COVENANT_NO_MODIFY_PATH=1 sh
    export PATH="$INSTALL_DIR/bin:$PATH"
    exec covenant "$@"
fi
