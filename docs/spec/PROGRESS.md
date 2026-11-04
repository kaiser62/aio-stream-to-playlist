# PROGRESS (live status)

_Last updated: 2026-11-04 by Claude Code. Update this file whenever state changes._
Legend: DONE = built and verified by running it. DOING = in flight. UNVERIFIED = written but not run. TODO.

## Snapshot
- CI on `main` is green: core tests on ubuntu/macos/windows, Tauri desktop builds on all three.
- Local build works: `build/build.sh app` produced `build/dist/Companion.app` (6.8 MiB) and `Companion_0.1.0_aarch64.dmg` (3 MiB). Unsigned, not yet launched by hand.
- Last commit 0a5283c on main. Uncommitted: embedded macOS player, player window UI, Player tab.

## Done
- [DONE] Python prototype preserved in `legacy/` (own CI workflow).
- [DONE] Rust `core/`: stream parsing, dedupe, playlists, TorBox client, multi-addon Stremio client, config, send jobs. 15 tests (unit + wiremock e2e) pass.
- [DONE] Tauri 2 shell `src-tauri/` with all addon/search/send/playlist commands; old UI ported behind an `invoke` router.
- [DONE] GitHub Actions: native workflow + legacy workflow. Contained local toolchain in `build/`.
- [DONE] `core/src/player.rs`: command set, IINA-style key map, `input.conf` generator, mpv JSON-IPC encode/parse, resume store. 22 unit tests pass.

- [DONE] `core/tests/mpv_ipc.rs`: headless real-mpv test (skips if mpv missing, unix only). Proves the generated `input.conf` loads, SPACE pauses via the key map, and IPC `cycle pause` resumes.

- [DONE, partly verified] **UI overhaul** (designer subagent + `ui-ux-pro-max`): new identity (ink/amber, serif headings, system fonts, tokens, light/dark/System), sidebar/rail/bottom-tab nav, redesigned Search/Results/Jobs/Playlists/Addons/Settings, in-app dialogs, `ui/mock.js` (`?mock=1`), new `icon.svg` and regenerated `icon.png`. Verified in the browser mock on phone width; tablet/desktop spot-checked by the designer. **Not run inside the real Tauri app.** `Play in app` calls `player_open`.

## Doing
- [DONE, partly verified] **Built-in player, macOS** (run in `build/build.sh dev`): player window = transparent Tauri window + mpv's own borderless window attached underneath as a child (`src-tauri/src/embed.rs`; mpv `wid` is ignored by the macOS VO). libmpv is dlopen'd (`mpvlib.rs`). Verified by screenshot: video renders, IINA-style OSC/title bar overlay shows over it, window opens from the new **Player tab** (open path/URL).
- [UNVERIFIED] In the real app: keyboard shortcuts (a Shift+Left test on a clip already at EOF showed no change, inconclusive), seek bar drag, sidebar (Playlist/Chapters/Quick Settings), context menu, OSD, fullscreen, pin-on-top, resize sync, resume, EOF behaviour (player sits idle at the end instead of closing or offering replay).
- [DONE] `ui/player.html|js|css`: OSC (time, seek with hover tooltip/chapter ticks/A-B loop, volume, prev/-10/play/+10/next, speed pill, playlist + quick-settings buttons), top-left OSD, title bar with pin, auto-hide chrome, right sidebar (Playlist, Chapters, Video/Audio/Subtitles quick settings), context menu, play/pause flash, buffering spinner, click/dblclick/wheel gestures, key map from `player_keys`. `?mock=1` demo mode (browser-checked).
- [DONE] Main UI **Player tab**: open path/URL, Now playing mini controls, Continue watching (resume from `player_recent`), shortcut cheat sheet. Player-specific settings not built yet.
- Commands: `player_open/play/props/set/exec/cmd/close/keys/recent/recent_remove`; events `player://prop {name,value}`, `player://state`, `player://closed`. Needs `app.macOSPrivateApi` (transparent window): Mac App Store risk, DMG/notarized is fine.

## Next (in order)
1. Verify the UNVERIFIED list above on a longer clip (use a 10 min test file): keys, seek, sidebar, fullscreen, resize, resume; fix EOF behaviour; prettify key names in the cheat sheet (`Shift+LEFT` -> symbols).
2. Window aspect-follow, rounded corners on the mpv child, player settings (hwdec, default speed, resume on/off) in the Player tab.
3. Windows (child HWND) and Linux embedding; mobile playback strategy.
4. Extend the mpv test with a range-capable HTTP server; TorBox link expiry refresh (store torrent/file id on `Entry`).
5. LGPL libmpv build, bundling, signing, notarization (see PLAYER.md).
6. Store readiness: name/trademark check, `cargo deny`, privacy policy, keychain storage, signing; Android/iOS CI.

## Known gaps / risks
- Only Xcode Command Line Tools installed locally: no iOS builds; no Android SDK. Mobile is CI-only until set up.
- Config (TorBox key, addon URLs) is currently plain JSON with 0600 perms, not the OS keychain.
- mpv is found via PATH plus common install dirs; if absent the player returns an error telling the user to install it.
- macOS build is unsigned/un-notarized.

## Changelog
- 2026-11-04: embedded macOS player (child-window trick), IINA-style overlay, Player tab; real-app check partial.
- 2026-11-04: added player core + mpv backend (unverified at runtime), spec kit, `ui-ux-pro-max` skill; started UI overhaul.
- 2026-11-03: CI green on all OSes; local `build/` toolchain; first local `.app`/`.dmg`.
