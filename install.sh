#!/bin/bash
set -e

# Port Tray Installer
# Usage: curl -fsSL https://raw.githubusercontent.com/try-samuel/port-tray/main/install.sh | bash

REPO="try-samuel/port-tray"
INSTALL_DIR="$HOME/.local/bin"
APP_NAME="port-tray"
ALIAS_NAME="findports"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info() { echo -e "${GREEN}[INFO]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Detect OS and architecture
detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"
    
    case "$OS" in
        Darwin) OS="macos" ;;
        Linux) OS="linux" ;;
        *) error "Unsupported OS: $OS. Only macOS and Linux are supported." ;;
    esac
    
    case "$ARCH" in
        x86_64|amd64) ARCH="x86_64" ;;
        arm64|aarch64) ARCH="aarch64" ;;
        *) error "Unsupported architecture: $ARCH" ;;
    esac
    
    # Linux only supports x86_64 for now
    if [ "$OS" = "linux" ] && [ "$ARCH" = "aarch64" ]; then
        error "Linux ARM64 is not yet supported. Please use x86_64."
    fi
    
    PLATFORM="${OS}_${ARCH}"
    info "Detected platform: $PLATFORM"
}

# Get latest release version
get_latest_version() {
    info "Fetching latest release..."
    VERSION=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    if [ -z "$VERSION" ]; then
        error "Could not determine latest version. Check your internet connection."
    fi
    info "Latest version: $VERSION"
}

# Download and install
install() {
    # Strip 'v' prefix from version for asset names
    VERSION_NUM="${VERSION#v}"
    
    # Construct download URL based on platform
    # Actual Tauri naming from release:
    #   macOS: Port.Tray_1.0.0_aarch64.dmg, Port.Tray_1.0.0_x64.dmg
    #   Linux: Port.Tray_1.0.0_amd64.deb
    case "$PLATFORM" in
        macos_x86_64)
            ASSET_NAME="Port.Tray_${VERSION_NUM}_x64.dmg"
            ;;
        macos_aarch64)
            ASSET_NAME="Port.Tray_${VERSION_NUM}_aarch64.dmg"
            ;;
        linux_x86_64)
            ASSET_NAME="Port.Tray_${VERSION_NUM}_amd64.deb"
            ;;
    esac
    
    DOWNLOAD_URL="https://github.com/$REPO/releases/download/$VERSION/$ASSET_NAME"
    
    info "Downloading $ASSET_NAME..."
    TEMP_DIR=$(mktemp -d)
    trap "rm -rf $TEMP_DIR" EXIT
    
    curl -fsSL "$DOWNLOAD_URL" -o "$TEMP_DIR/$ASSET_NAME" || error "Download failed. Release may not exist yet."
    
    # Install based on platform
    if [ "$OS" = "macos" ]; then
        info "Installing Port Tray.app..."
        
        # Mount DMG
        cd "$TEMP_DIR"
        MOUNT_POINT=$(hdiutil attach "$ASSET_NAME" -nobrowse -quiet | grep -o '/Volumes/.*' | head -1)
        
        if [ -z "$MOUNT_POINT" ]; then
            error "Failed to mount DMG"
        fi
        
        # Copy app to Applications
        if [ -d "/Applications/Port Tray.app" ]; then
            warn "Removing existing installation..."
            rm -rf "/Applications/Port Tray.app"
        fi
        
        cp -R "$MOUNT_POINT/Port Tray.app" /Applications/
        
        # Unmount DMG
        hdiutil detach "$MOUNT_POINT" -quiet 2>/dev/null || true
        
        # Remove quarantine attribute (bypass Gatekeeper for unsigned app)
        xattr -cr "/Applications/Port Tray.app" 2>/dev/null || true
        
        info "Installed to /Applications/Port Tray.app"
        
        # Create CLI wrapper
        mkdir -p "$INSTALL_DIR"
        cat > "$INSTALL_DIR/$APP_NAME" << 'EOF'
#!/bin/bash
open -a "Port Tray"
EOF
        chmod +x "$INSTALL_DIR/$APP_NAME"
        
        # Create alias
        ln -sf "$INSTALL_DIR/$APP_NAME" "$INSTALL_DIR/$ALIAS_NAME"
        
    elif [ "$OS" = "linux" ]; then
        info "Installing via dpkg..."
        sudo dpkg -i "$TEMP_DIR/$ASSET_NAME" || sudo apt-get install -f -y
        
        # Create alias
        mkdir -p "$INSTALL_DIR"
        ln -sf "/usr/bin/$APP_NAME" "$INSTALL_DIR/$ALIAS_NAME" 2>/dev/null || \
            sudo ln -sf "/usr/bin/$APP_NAME" "/usr/local/bin/$ALIAS_NAME"
    fi
}

# Add to PATH if needed
setup_path() {
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        warn "$INSTALL_DIR is not in your PATH"
        
        SHELL_NAME=$(basename "$SHELL")
        case "$SHELL_NAME" in
            zsh)  RC_FILE="$HOME/.zshrc" ;;
            bash) RC_FILE="$HOME/.bashrc" ;;
            *)    RC_FILE="$HOME/.profile" ;;
        esac
        
        if ! grep -q "$INSTALL_DIR" "$RC_FILE" 2>/dev/null; then
            echo "" >> "$RC_FILE"
            echo "# Port Tray" >> "$RC_FILE"
            echo "export PATH=\"\$PATH:$INSTALL_DIR\"" >> "$RC_FILE"
            info "Added $INSTALL_DIR to PATH in $RC_FILE"
            warn "Run 'source $RC_FILE' or restart your terminal"
        fi
    fi
}

main() {
    echo ""
    echo "  ╔═══════════════════════════════════╗"
    echo "  ║       Port Tray Installer         ║"
    echo "  ╚═══════════════════════════════════╝"
    echo ""
    
    detect_platform
    get_latest_version
    install
    setup_path
    
    echo ""
    info "Installation complete! 🎉"
    echo ""
    echo "  Run with: ${GREEN}findports${NC}"
    echo ""
}

main "$@"
