#!/bin/bash
set -euo pipefail

# --- CONFIGURATION ---
APP_NAME="ssbt-tool"
HASH="${HASH:-unknown}"
INSTALL_DIR="/usr/local/bin"
TMP_DIR="$(mktemp -d)"
ARCH="$(uname -m)"
RELEASE_DIR="$(cd "$(dirname "$0")" && pwd)"

# --- DETECT ARCH ---
case "$ARCH" in
  x86_64) PLATFORM="macos-x86_64" ;;
  arm64)  PLATFORM="macos-arm64" ;;
  *)
    echo "❌ Unsupported architecture: $ARCH"
    exit 1
    ;;
esac

# --- INSTALLATION ---
echo "🍎 Installing $APP_NAME ($HASH) for $PLATFORM..."
echo "   Source directory: $RELEASE_DIR"
echo "   Target directory: $INSTALL_DIR"
echo

# Verify binary presence
if [[ ! -f "$RELEASE_DIR/$APP_NAME" ]]; then
  echo "❌ Binary not found: $RELEASE_DIR/$APP_NAME"
  exit 1
fi

# Ensure sudo if needed
if [[ ! -w "$INSTALL_DIR" ]]; then
  echo "⚙️  Root privileges required to install into $INSTALL_DIR"
  sudo install -m 755 "$RELEASE_DIR/$APP_NAME" "$INSTALL_DIR/$APP_NAME"
else
  install -m 755 "$RELEASE_DIR/$APP_NAME" "$INSTALL_DIR/$APP_NAME"
fi

# --- POST-INSTALL ---
echo
echo "✅ $APP_NAME installed successfully!"
echo "   Version hash: $HASH"
echo "   Installed to: $(command -v $APP_NAME)"
echo

# --- macOS NOTICE ---
echo "🔐 Note for macOS users:"
echo "  If you see a security warning ('app cannot be opened because it is from an unidentified developer'),"
echo "  you can allow it once via:"
echo "    xattr -d com.apple.quarantine $(command -v $APP_NAME)"
echo
echo "To verify installation, run:"
echo "  $APP_NAME --help"

# --- CLEANUP ---
rm -rf "$TMP_DIR"
