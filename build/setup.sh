#!/usr/bin/env bash
# One-time (idempotent) local toolchain install, contained in build/.
set -euo pipefail
source "$(cd "$(dirname "$0")" && pwd)/env.sh"
mkdir -p "$BUILD_DIR/downloads"

if [ ! -x "$CARGO_HOME/bin/cargo" ]; then
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64)  T=aarch64-apple-darwin ;;
    Darwin-x86_64) T=x86_64-apple-darwin ;;
    Linux-x86_64)  T=x86_64-unknown-linux-gnu ;;
    Linux-aarch64) T=aarch64-unknown-linux-gnu ;;
    *) echo "Unsupported platform $(uname -sm); install Rust manually" >&2; exit 1 ;;
  esac
  echo "==> Downloading rustup-init ($T) from static.rust-lang.org"
  curl --proto '=https' --tlsv1.2 -fsSL "https://static.rust-lang.org/rustup/dist/$T/rustup-init" -o "$BUILD_DIR/downloads/rustup-init"
  chmod +x "$BUILD_DIR/downloads/rustup-init"
  "$BUILD_DIR/downloads/rustup-init" -y --no-modify-path --profile minimal --default-toolchain stable
fi

if [ ! -x "$BUILD_DIR/node/node_modules/.bin/tauri" ]; then
  echo "==> Installing Tauri CLI into build/node"
  mkdir -p "$BUILD_DIR/node"
  (cd "$BUILD_DIR/node" && npm init -y >/dev/null 2>&1 && npm install --no-audit --no-fund @tauri-apps/cli@2)
fi
echo "==> Toolchain ready"
cargo --version; rustc --version; tauri --version
