#!/bin/bash
set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info() { echo -e "${GREEN}==>${NC} $1"; }
warn() { echo -e "${YELLOW}Warning:${NC} $1"; }
error() { echo -e "${RED}Error:${NC} $1"; exit 1; }

# Default install directory
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
REPO_URL="https://raw.githubusercontent.com/yusukeshib/nixy/main/nixy"

# Check dependencies
check_dependencies() {
    local missing=()

    if ! command -v nix &> /dev/null; then
        missing+=("nix")
    fi

    if ! command -v jq &> /dev/null; then
        missing+=("jq")
    fi

    if [ ${#missing[@]} -ne 0 ]; then
        warn "Missing dependencies: ${missing[*]}"
        echo "  Please install them before using nixy:"
        echo "  - Nix: https://nixos.org/download.html"
        echo "  - jq: Install via your package manager or 'nix profile install nixpkgs#jq'"
        echo ""
    fi
}

# Create install directory if needed
ensure_install_dir() {
    if [ ! -d "$INSTALL_DIR" ]; then
        info "Creating $INSTALL_DIR"
        mkdir -p "$INSTALL_DIR"
    fi
}

# Check if directory is in PATH
check_path() {
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        warn "$INSTALL_DIR is not in your PATH"
        echo "  Add it to your shell profile:"
        echo ""
        echo "  # For bash (~/.bashrc)"
        echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
        echo ""
        echo "  # For zsh (~/.zshrc)"
        echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
        echo ""
    fi
}

# Download and install nixy
install_nixy() {
    local target="$INSTALL_DIR/nixy"

    # Check if nixy is already installed somewhere
    if command -v nixy &> /dev/null; then
        local existing=$(command -v nixy)
        existing=$(realpath "$existing" 2>/dev/null || echo "$existing")

        # If installed elsewhere, update that location instead
        if [[ "$existing" != "$target" ]]; then
            info "Found existing nixy at $existing"
            target="$existing"
        fi
    fi

    info "Downloading nixy to $target"

    # Ensure target directory exists
    local target_dir=$(dirname "$target")
    if [[ ! -d "$target_dir" ]]; then
        mkdir -p "$target_dir"
    fi

    if command -v curl &> /dev/null; then
        curl -fsSL "$REPO_URL" -o "$target"
    elif command -v wget &> /dev/null; then
        wget -qO "$target" "$REPO_URL"
    else
        error "Neither curl nor wget found. Please install one of them."
    fi

    chmod +x "$target"
}

main() {
    echo ""
    echo "Installing nixy - Homebrew-style wrapper for Nix"
    echo ""

    check_dependencies
    ensure_install_dir
    install_nixy
    check_path

    echo ""
    info "nixy installed successfully!"
    echo ""
    echo "  Get started:"
    echo "    nixy install <package>   # Install a package"
    echo "    nixy search <query>      # Search for packages"
    echo ""
    echo "  For more info: https://github.com/yusukeshib/nixy"
    echo ""
}

main
