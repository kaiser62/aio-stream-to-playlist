# BUILD AND CI

## Local (everything contained in `build/`)
```bash
build/setup.sh            # installs rustup/cargo, tauri-cli, node into build/ (first time)
source build/env.sh       # RUSTUP_HOME, CARGO_HOME, CARGO_TARGET_DIR -> build/
build/build.sh test       # cargo test -p companion-core + e2e
build/build.sh dev        # tauri dev
build/build.sh app        # tests, node --check, icon generation, tauri build, copy to build/dist
```
Outputs: `build/target/release/bundle/...`, copies in `build/dist` (git-ignored).
Prereqs on this Mac: Xcode Command Line Tools only (no full Xcode, no Android SDK).

## CI (`.github/workflows/native.yml`)
- Job `core-tests`: ubuntu, macos, windows: `cargo test -p companion-core`.
- Job `desktop-build`: macos-latest, windows-latest, ubuntu-22.04: `npx @tauri-apps/cli@2 icon ui/icon.png` then `tauri build`; uploads bundles.
- Legacy: `.github/workflows/legacy-python.yml`.
- TODO: Android (APK/AAB) and iOS simulator jobs; headless mpv integration job (`xvfb-run`); libmpv install/cache per OS; signing/notarization secrets; release on tags.

## Checking CI
```bash
gh run list --limit 3
gh run watch
```
(Retry on transient network errors.)

## Release (TODO)
Version scheme shared by all platforms, one bundle id `app.companion.streamplaylist` (rename before store submission), Apple Developer ($99/yr, notarization + TestFlight), Google Play ($25, AAB with upload key, internal track), GitHub Releases, optional Homebrew cask/winget/F-Droid.
