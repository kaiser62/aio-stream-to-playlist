# PROGRESS (live status)

_Last updated: 2026-11-04 by Claude Code. Update this file whenever state changes._
Legend: DONE = built and verified by running it. DOING = in flight. UNVERIFIED = written but not run. TODO.

## Snapshot
- CI on `main` is green: core tests on ubuntu/macos/windows, Tauri desktop builds on all three.
- Local build works: `build/build.sh app` produced `build/dist/Companion.app` (6.8 MiB) and `Companion_0.1.0_aarch64.dmg` (3 MiB). Unsigned, not yet launched by hand.
- Uncommitted work in tree: player core + mpv backend (below), UI redesign in progress, `.claude/skills/ui-ux-pro-max`, spec kit.

## Done
- [DONE] Python prototype preserved in `legacy/` (own CI workflow).
- [DONE] Rust `core/`: stream parsing, dedupe, playlists, TorBox client, multi-addon Stremio client, config, send jobs. 15 tests (unit + wiremock e2e) pass.
- [DONE] Tauri 2 shell `src-tauri/` with all addon/search/send/playlist commands; old UI ported behind an `invoke` router.
- [DONE] GitHub Actions: native workflow + legacy workflow. Contained local toolchain in `build/`.
- [DONE] `core/src/player.rs`: command set, IINA-style key map, `input.conf` generator, mpv JSON-IPC encode/parse, resume store. 8 unit tests pass.

- [DONE] `core/tests/mpv_ipc.rs`: headless real-mpv test (skips if mpv missing, unix only). Proves the generated `input.conf` loads, SPACE pauses via the key map, and IPC `cycle pause` resumes.

- [DONE, partly verified] **UI overhaul** (designer subagent + `ui-ux-pro-max`): new identity (ink/amber, serif headings, system fonts, tokens, light/dark/System), sidebar/rail/bottom-tab nav, redesigned Search/Results/Jobs/Playlists/Addons/Settings, in-app dialogs, `ui/mock.js` (`?mock=1`), new `icon.svg` and regenerated `icon.png`. Verified in the browser mock on phone width; tablet/desktop spot-checked by the designer. **Not run inside the real Tauri app.** `Play in app` calls `player_open`.

## Doing
- [UNVERIFIED] `src-tauri/src/player.rs` (spawn mpv, IPC reader thread, resume, events `player://state`, `player://closed`) and commands `player_open/cmd/close/keys`. `cargo check -p companion` passes; **never run against a real mpv yet**.

## Next (in order)
1. Run the player for real with a window: `build/build.sh dev`, open a playlist with "Play in app", confirm the window, resume, and `player://state` events (the key map and IPC are already proven headless).
2. Extend the mpv test with a local range-capable HTTP server to check seeking sends `Range:` requests.
3. Run the new UI inside the real app (`build/build.sh dev`) and fix regressions; finish light-mode and desktop-sidebar screenshots on every view; check reduced motion and landscape.
4. Commit (include `Cargo.lock`), push, confirm CI green.
5. TorBox link expiry: store torrent/file id on `Entry`, refresh link and resume on playback error.
6. Phase 2 player: embedded libmpv on macOS (spike first). Needs LGPL libmpv build decision.
7. Store readiness: neutral name/trademark check, license + `cargo deny`, privacy policy, keychain storage for TorBox key and addon URLs, signing.
8. Mobile: Android/iOS CI jobs; mobile playback (system player fallback).

## Known gaps / risks
- Only Xcode Command Line Tools installed locally: no iOS builds; no Android SDK. Mobile is CI-only until set up.
- Config (TorBox key, addon URLs) is currently plain JSON with 0600 perms, not the OS keychain.
- mpv is found via PATH plus common install dirs; if absent the player returns an error telling the user to install it.
- macOS build is unsigned/un-notarized.

## Changelog
- 2026-11-04: added player core + mpv backend (unverified at runtime), spec kit, `ui-ux-pro-max` skill; started UI overhaul.
- 2026-11-03: CI green on all OSes; local `build/` toolchain; first local `.app`/`.dmg`.
