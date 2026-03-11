#!/usr/bin/env bash
set -euo pipefail

REPO="bloccooo/envisible"
BIN="envi"
INSTALL_DIR="/usr/local/bin"

# Detect OS and arch
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)
    TARGET="linux-x64"
    ;;
  Darwin)
    case "$ARCH" in
      arm64) TARGET="darwin-arm64" ;;
      x86_64) TARGET="darwin-x64" ;;
      *) echo "Unsupported architecture: $ARCH" && exit 1 ;;
    esac
    ;;
  *)
    echo "Unsupported OS: $OS"
    exit 1
    ;;
esac

# Resolve latest release tag
echo "Fetching latest release..."
TAG="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"

if [ -z "$TAG" ]; then
  echo "Could not determine latest release tag."
  exit 1
fi

URL="https://github.com/$REPO/releases/download/$TAG/$BIN-$TARGET"

echo "Installing $BIN $TAG ($TARGET)..."
curl -fsSL "$URL" -o "/tmp/$BIN"
chmod +x "/tmp/$BIN"

# Install — try without sudo first, fall back to sudo
if [ -w "$INSTALL_DIR" ]; then
  mv "/tmp/$BIN" "$INSTALL_DIR/$BIN"
else
  echo "Sudo required to install to $INSTALL_DIR"
  sudo mv "/tmp/$BIN" "$INSTALL_DIR/$BIN"
fi

echo "$BIN installed to $INSTALL_DIR/$BIN"
$BIN --help 2>/dev/null || true
