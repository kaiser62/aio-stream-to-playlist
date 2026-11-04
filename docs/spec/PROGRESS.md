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
- [DONE, verified on this Mac] **Built-in player, macOS, inside the main window**: the Player tab hosts `player.html?embed=1` in an iframe over a transparent main window (`transparent: true` + `macOSPrivateApi`). mpv's own borderless window is a child window below the main window; the UI reports the iframe rect via `player_set_rect` and Rust places the mpv window on it (`embed.rs`, uses `contentLayoutRect`). A CSS `clip-path` hole in the page background lets the video show through. Verified by screenshot: local 10 min clip and a real TorBox playlist (Reacher S01) play in the tab with the IINA-style bar; click = pause and Shift+Right (+60 s) work.
- [UNVERIFIED] In the real app: seek-bar drag, sidebar (Playlist/Chapters/Quick Settings), context menu, fullscreen (`pfs` mode + window fullscreen), pin, volume wheel, resume, EOF behaviour, window resize/scroll sync, black ring around the bar (box-shadow over transparency).
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
- 2026-11-04: Watching tab now shows series/movie metadata (poster/episode still, title, S/E + episode name, year, rating, genres, summary tooltip) via new `title_meta` command (user's metadata addon, Cinemeta by default), cached in localStorage. Verified live on real items.
- 2026-11-04: player Video sidebar: Anime4K presets (A/B/C x HQ/Fast, bloc97/Anime4K MIT shaders embedded in `src-tauri/shaders`, extracted to app data, applied by `player_filter`), motion smoothing (Off/Light/Blend = mpv interpolation + display-resample), scaler chips, deband. Verified live: mpv reports the glsl-shaders list, interpolation=true, video-sync=display-resample, tscale=linear. UNVERIFIED: visual quality/perf on real anime, HQ presets on weak GPUs. Not implemented: motion-compensated SVP/RIFE (needs VapourSynth + SVP; SVP 4 installed here but not wired). Fix: video hole in the page is only cut once a video is actually playing (a failed/expired link no longer shows the desktop through the stage).
- 2026-11-04: Watching tab (Netflix-style continue watching from recent.json: cards, progress, time left, remove, resume). Fixed control-bar ghost ring (no backdrop blur when embedded), iframe tooltip, bar squeezed by sidebar, desktop leaking through before first frame. Resume verified on real TorBox video.
- 2026-11-04: player moved from a separate window into the Player tab (iframe + child-window placement); fixed stale `player://closed` from a replaced session; verified with real TorBox video.
- 2026-11-04: embedded macOS player (child-window trick), IINA-style overlay, Player tab; real-app check partial.
- 2026-11-04: added player core + mpv backend (unverified at runtime), spec kit, `ui-ux-pro-max` skill; started UI overhaul.
- 2026-11-03: CI green on all OSes; local `build/` toolchain; first local `.app`/`.dmg`.
- 2026-11-04 (later): added 60/90/120 fps target chips (ffmpeg `minterpolate`, blend or motion-comp; `player_fps`), player loading state (opaque stage + spinner until first frame), Liquid Glass layer (`ui/glass.css`, `ui/player-glass.css`; Settings > Appearance: App/Player Glass vs Classic + Opacity slider `--gl`), fullscreen (`html.pfs` layout; macOS uses simple fullscreen via `window_fullscreen` because native fullscreen put mpv's child window above the HTML controls), debug overlay (key I / context menu / Quick settings: fps, dropped frames, codec, hwdec, A/V sync, filters), trackpad + touch swipes (horizontal seek, vertical volume; wheel listener on the whole page), Logs tab (in-memory ring buffer `src-tauri/src/applog.rs`, URLs reduced to host, UI errors forwarded), URL box hidden while something is loaded in the Player tab. Verified live: fps chips reach the UI, glass look, Logs tab renders. UNVERIFIED: simple fullscreen + controls visible over video (previous native-fullscreen build hid them), swipe seek/volume on a real trackpad, debug overlay values, Opacity slider look, Classic revert, Logs content during playback, fps targets' CPU cost.

- OSC "controls do not work": live test in windowed embed: play/pause and +10 s respond (0:52 -> 1:04). Likely cause = macOS drops the first click on a non-key window; set `acceptFirstMouse: true` in tauri.conf.json (UNVERIFIED, needs app restart). Fullscreen path still UNVERIFIED.
