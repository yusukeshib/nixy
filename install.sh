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

# Configuration
REPO="yusukeshib/nixy"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
BINARY_NAME="nixy"

# Detect OS and architecture
detect_platform() {
    local os arch

    case "$(uname -s)" in
        Linux*)  os="linux" ;;
        Darwin*) os="darwin" ;;
        *)       error "Unsupported OS: $(uname -s)" ;;
    esac

    case "$(uname -m)" in
        x86_64)  arch="x86_64" ;;
        aarch64) arch="aarch64" ;;
        arm64)   arch="aarch64" ;;
        *)       error "Unsupported architecture: $(uname -m)" ;;
    esac

    echo "${arch}-${os}"
}

# Check if nix is installed
check_nix() {
    if ! command -v nix &> /dev/null; then
        warn "Nix is not installed"
        echo "  nixy requires Nix to function. Install it from:"
        echo "  https://nixos.org/download.html"
        echo ""
        return 1
    fi
    return 0
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
        echo "  # For fish (~/.config/fish/config.fish)"
        echo "  set -gx PATH \$HOME/.local/bin \$PATH"
        echo ""
    fi
}

# Try to download pre-built binary from GitHub releases
try_download_binary() {
    local platform="$1"
    local target="$INSTALL_DIR/$BINARY_NAME"
    local release_url="https://github.com/$REPO/releases/latest/download/nixy-${platform}"

    info "Attempting to download pre-built binary..."

    # Create install directory
    mkdir -p "$INSTALL_DIR"

    # Try to download
    local http_code
    if command -v curl &> /dev/null; then
        http_code=$(curl -fsSL -w "%{http_code}" -o "$target.tmp" "$release_url" 2>/dev/null) || http_code="000"
    elif command -v wget &> /dev/null; then
        if wget -q -O "$target.tmp" "$release_url" 2>/dev/null; then
            http_code="200"
        else
            http_code="000"
        fi
    else
        return 1
    fi

    if [[ "$http_code" == "200" ]] && [[ -f "$target.tmp" ]]; then
        mv "$target.tmp" "$target"
        chmod +x "$target"
        return 0
    else
        rm -f "$target.tmp"
        return 1
    fi
}

# Build from source using nix
build_with_nix() {
    info "Building from source with nix..."

    if ! command -v nix &> /dev/null; then
        return 1
    fi

    # Create install directory
    mkdir -p "$INSTALL_DIR"

    # Build with nix and copy binary
    local result
    result=$(nix --extra-experimental-features "nix-command flakes" build "github:$REPO" --no-link --print-out-paths 2>&1) || return 1

    if [[ -f "$result/bin/nixy" ]]; then
        cp "$result/bin/nixy" "$INSTALL_DIR/$BINARY_NAME"
        chmod +x "$INSTALL_DIR/$BINARY_NAME"
        return 0
    fi

    return 1
}

main() {
    echo ""
    echo "Installing nixy - Homebrew-style wrapper for Nix"
    echo ""

    # Check nix first (required for nixy to work)
    check_nix || true

    local platform
    platform=$(detect_platform)
    info "Detected platform: $platform"

    # Try installation methods in order of preference
    local installed=false

    # 1. Try pre-built binary (fastest)
    if try_download_binary "$platform"; then
        installed=true
        info "Installed pre-built binary"
    fi

    # 2. Try nix build (if nix available)
    if [[ "$installed" == "false" ]] && command -v nix &> /dev/null; then
        if build_with_nix; then
            installed=true
            info "Built and installed with nix"
        fi
    fi

    # 3. Provide manual instructions if all methods failed
    if [[ "$installed" == "false" ]]; then
        error "Could not install nixy automatically.

Please install manually using one of these methods:

  # Using nix profile
  nix profile install github:$REPO

  # Using nix run (without installing)
  nix run github:$REPO -- --help
"
    fi

    check_path

    echo ""
    info "nixy installed successfully!"
    echo ""
    echo "  Setup your shell (add to your shell's rc file):"
    echo "    eval \"\$(nixy config bash)\"   # for bash"
    echo "    eval \"\$(nixy config zsh)\"    # for zsh"
    echo "    nixy config fish | source     # for fish"
    echo ""
    echo "  Get started:"
    echo "    nixy install <package>   # Install a package"
    echo "    nixy search <query>      # Search for packages"
    echo "    nixy list                # List installed packages"
    echo ""
    echo "  For more info: https://github.com/$REPO"
    echo ""
}

main "$@"
