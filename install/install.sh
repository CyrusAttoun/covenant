#!/bin/sh
# Covenant installer
# Usage: curl -fsSL https://raw.githubusercontent.com/Cyronius/covenant/master/install/install.sh | sh
#
# Environment variables:
#   COVENANT_INSTALL         - Installation directory (default: $HOME/.covenant)
#   COVENANT_VERSION         - Specific version to install (default: latest)
#   COVENANT_NO_MODIFY_PATH  - Set to 1 to skip PATH modification

set -eu

REPO="Cyronius/covenant"

main() {
    platform=$(detect_platform)
    arch=$(detect_arch)
    install_dir="${COVENANT_INSTALL:-$HOME/.covenant}"
    bin_dir="$install_dir/bin"

    version=$(resolve_version)
    url="https://github.com/${REPO}/releases/download/v${version}/covenant-${version}-${platform}-${arch}.tar.gz"

    info "Installing" "Covenant v${version} (${platform}/${arch})"

    tmpdir=$(mktemp -d)
    trap 'rm -rf "$tmpdir"' EXIT

    download "$url" "$tmpdir/covenant.tar.gz"

    mkdir -p "$bin_dir"
    tar -xzf "$tmpdir/covenant.tar.gz" -C "$bin_dir"
    chmod +x "$bin_dir/covenant"

    configure_path "$bin_dir"

    echo ""
    info "Installed" "Covenant to $bin_dir/covenant"
    echo ""
    echo "  Run 'covenant --help' to get started."
    echo ""

    if ! echo ":$PATH:" | grep -q ":$bin_dir:"; then
        echo "  To update PATH for this session, run:"
        echo "    export PATH=\"$bin_dir:\$PATH\""
        echo ""
    fi

    echo "  To uninstall: rm -rf $install_dir"
}

detect_platform() {
    case "$(uname -s)" in
        Linux*)  echo "linux" ;;
        Darwin*) echo "darwin" ;;
        MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
        *) error "Unsupported operating system: $(uname -s)" ;;
    esac
}

detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64)  echo "x86_64" ;;
        aarch64|arm64) echo "aarch64" ;;
        *) error "Unsupported architecture: $(uname -m)" ;;
    esac
}

resolve_version() {
    if [ -n "${COVENANT_VERSION:-}" ]; then
        echo "$COVENANT_VERSION"
        return
    fi

    if command -v curl >/dev/null 2>&1; then
        version=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | \
            grep '"tag_name"' | sed -E 's/.*"v?([^"]+)".*/\1/')
    elif command -v wget >/dev/null 2>&1; then
        version=$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | \
            grep '"tag_name"' | sed -E 's/.*"v?([^"]+)".*/\1/')
    else
        error "curl or wget is required. Alternatively, set COVENANT_VERSION."
    fi

    if [ -z "$version" ]; then
        error "Failed to determine latest version. Set COVENANT_VERSION manually."
    fi

    echo "$version"
}

download() {
    url="$1"
    dest="$2"

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --retry 3 "$url" -o "$dest"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "$url" -O "$dest"
    else
        error "curl or wget is required to download Covenant."
    fi
}

configure_path() {
    bin_dir="$1"

    # Skip if already in PATH
    if echo ":$PATH:" | grep -q ":$bin_dir:"; then
        return
    fi

    # Skip if user opted out
    if [ "${COVENANT_NO_MODIFY_PATH:-0}" = "1" ]; then
        return
    fi

    path_export="export PATH=\"\$HOME/.covenant/bin:\$PATH\""

    # Detect shell config file
    if [ -n "${ZDOTDIR:-}" ] && [ -f "$ZDOTDIR/.zshrc" ]; then
        config="$ZDOTDIR/.zshrc"
    elif [ -f "$HOME/.zshrc" ]; then
        config="$HOME/.zshrc"
    elif [ -f "$HOME/.bashrc" ]; then
        config="$HOME/.bashrc"
    elif [ -f "$HOME/.bash_profile" ]; then
        config="$HOME/.bash_profile"
    else
        config="$HOME/.profile"
    fi

    # Don't add if already present
    if grep -q ".covenant/bin" "$config" 2>/dev/null; then
        return
    fi

    echo "" >> "$config"
    echo "# Covenant" >> "$config"
    echo "$path_export" >> "$config"

    info "Updated" "$config"
}

info() {
    printf '\033[1;32m%s\033[0m %s\n' "$1" "$2"
}

error() {
    printf '\033[1;31merror\033[0m: %s\n' "$@" >&2
    exit 1
}

main
