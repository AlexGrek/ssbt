#!/bin/bash
set -euo pipefail

# --- CONFIGURATION ---
APP_NAME="ssbt-tool"
HASH="${HASH:-unknown}"
INSTALL_DIR="/usr/local/bin"
TMP_DIR="$(mktemp -d)"
ARCH="$(uname -m)"
RELEASE_DIR="$(dirname "$0")"

# --- DETECT ARCH ---
case "$ARCH" in
  x86_64) PLATFORM="linux-x86_64" ;;
  aarch64 | arm64) PLATFORM="linux-arm64" ;;
  *)
    echo "❌ Unsupported architecture: $ARCH"
    exit 1
    ;;
esac

# --- INSTALLATION ---
echo "📦 Installing $APP_NAME ($HASH) for $PLATFORM..."
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
echo "To verify, run:"
echo "  $APP_NAME --help"

# --- CLEANUP ---
rm -rf "$TMP_DIR"
