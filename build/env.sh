# Source me:  source build/env.sh
# Keeps the whole Rust toolchain, caches and outputs inside <project>/build (nothing in ~ or your shell profile).
BUILD_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
export PROJECT_DIR="$(dirname "$BUILD_DIR")"
export RUSTUP_HOME="$BUILD_DIR/rustup"
export CARGO_HOME="$BUILD_DIR/cargo"
export CARGO_TARGET_DIR="$BUILD_DIR/target"
export PATH="$CARGO_HOME/bin:$BUILD_DIR/node/node_modules/.bin:$PATH"
