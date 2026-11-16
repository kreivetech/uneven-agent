#!/usr/bin/env bash
set -e

echo "=================================================="
echo "        INSTALLING UNEVEN AI         "
echo "=================================================="

# Detect OS and Architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)
    OS_TARGET="linux"
    ;;
  Darwin)
    OS_TARGET="darwin"
    ;;
  *)
    echo "Unsupported OS: $OS"
    exit 1
    ;;
esac

case "$ARCH" in
  x86_64)
    ARCH_TARGET="x64"
    ;;
  arm64|aarch64)
    ARCH_TARGET="arm64"
    ;;
  *)
    echo "Unsupported architecture: $ARCH"
    exit 1
    ;;
esac

TARGET="${OS_TARGET}-${ARCH_TARGET}"

if [ "$TARGET" = "darwin-x64" ]; then
  echo "Error: macOS x64 is not supported. Please use Apple Silicon (arm64)."
  exit 1
fi

BIN_DIR="$HOME/.uneven/bin"
mkdir -p "$BIN_DIR"

# Clean up legacy binaries and companion modules from older versions
if [ -f "$BIN_DIR/uneven-ai" ] || [ -f "$BIN_DIR/uneven-core.uneven" ]; then
  echo "-> Cleaning up legacy files..."
  rm -f "$BIN_DIR/uneven-ai"
  rm -f "$BIN_DIR/uneven-core.uneven"
  rm -f "$BIN_DIR/uneven-core-${TARGET}.uneven"
  rm -f "$BIN_DIR/uneven_core.node"
  rm -f "$BIN_DIR/uneven.uneven"
fi

BIN_NAME="uneven"

DOWNLOAD_BASE="https://edge.uneven.dev/api/download"

echo "-> Fetching binary for $TARGET..."
curl -fsSL "${DOWNLOAD_BASE}?target=${TARGET}&asset=bin" -o "$BIN_DIR/$BIN_NAME"
chmod +x "$BIN_DIR/$BIN_NAME"


# Setup PATH
if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
  echo "-> Adding $BIN_DIR to PATH..."
  if [ -n "$ZSH_VERSION" ] || [ -f "$HOME/.zshrc" ]; then
    echo 'export PATH="$HOME/.uneven/bin:$PATH"' >> "$HOME/.zshrc"
    echo "-> Configured ~/.zshrc"
  elif [ -f "$HOME/.bashrc" ]; then
    echo 'export PATH="$HOME/.uneven/bin:$PATH"' >> "$HOME/.bashrc"
    echo "-> Configured ~/.bashrc"
  else
    echo 'export PATH="$HOME/.uneven/bin:$PATH"' >> "$HOME/.profile"
    echo "-> Configured ~/.profile"
  fi
fi

echo "=================================================="
echo " ✨ Installation complete! Start a new terminal  "
echo "    or run: export PATH=\"\$HOME/.uneven/bin:\$PATH\" "
echo "    Then run: uneven init                      "
echo "=================================================="
