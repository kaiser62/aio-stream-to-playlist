#!/usr/bin/env bash
# Usage: build/build.sh [test|dev|app]     (default: app)
#   test  run the Rust unit + e2e tests
#   dev   run the desktop app with live UI reload
#   app   test, then produce installers in build/dist
set -euo pipefail
source "$(cd "$(dirname "$0")" && pwd)/env.sh"
cd "$PROJECT_DIR"
[ -x "$CARGO_HOME/bin/cargo" ] || "$BUILD_DIR/setup.sh"

case "${1:-app}" in
  test) cargo test -p companion-core --all-targets ;;
  dev)  tauri icon ui/icon.png >/dev/null; tauri dev ;;
  app)
    cargo test -p companion-core --all-targets
    node --check ui/app.js
    tauri icon ui/icon.png >/dev/null
    tauri build
    rm -rf "$BUILD_DIR/dist" && mkdir -p "$BUILD_DIR/dist"
    find "$CARGO_TARGET_DIR/release/bundle" -maxdepth 2 \( -name '*.dmg' -o -name '*.app' -o -name '*.msi' -o -name '*.exe' -o -name '*.deb' -o -name '*.AppImage' \) -exec cp -R {} "$BUILD_DIR/dist/" \;
    echo "==> Output in build/dist:"; ls -1 "$BUILD_DIR/dist"
    ;;
  *) echo "usage: $0 [test|dev|app]" >&2; exit 2 ;;
esac
