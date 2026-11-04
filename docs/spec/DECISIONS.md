# DECISIONS (settled; change only with the user's say-so)

| Date | Decision | Why |
|---|---|---|
| 2026-11 | Stremio-addon based (AIOStreams first), TorBox as debrid | User's setup |
| 2026-11 | Rewrite from Python to **Rust + Tauri 2** (one core, desktop + iOS + Android, reuse web UI) | User wanted native apps on every device in Rust or React Native |
| 2026-11 | Python prototype kept in `legacy/` | Reference and fallback |
| 2026-11 | Support any Stremio-compatible addon with an addon manager | User request |
| 2026-11 | Open source (MIT), target Google Play + App Store + desktop | User direction; GPL conflicts with App Store |
| 2026-11 | Neutral working name "Companion for TorBox", unofficial notice | Trademark and store review |
| 2026-11 | No LAN server / PIN in native app; each device standalone | Native app has no server |
| 2026-11 | UI: clean-room recreation of AIOStreams-style layout, then redesign | AGPL code must not be copied |
| 2026-11 | Build contained in `build/`; CI in GitHub Actions | User request |
| 2026-11-04 | Player: mpv backend, IINA-style shortcuts; Phase 1 = external mpv window over JSON IPC, embedding later | Lowest risk first; see PLAYER.md |
| 2026-11-04 | UI work uses the `ui-ux-pro-max` skill (installed in `.claude/skills/`) | User request |
| 2026-11-04 | macOS embedding: mpv's `wid` is ignored by its macOS VO, so we let mpv open its own borderless window and attach it as a child window *below* our transparent Tauri player window (HTML overlay on top). Needs `macOSPrivateApi` | Verified working by screenshot; App Store (Mac) risk noted, DMG fine |
| 2026-11-04 | libmpv loaded at runtime (dlopen), not linked | Keeps GPL/LGPL choice a packaging decision |
| 2026-11-04 | Player UI is an in-window HTML overlay (`ui/player.*`) driven by `player://prop` events and allowlisted `player_exec/set` | Clean-room IINA look/behaviour, one codebase for all desktop OSes |
| 2026-11-04 | Player lives in the Player tab of the main window, not a separate window (user request). Main window is transparent; page background is clipped around the video rect | Verified on macOS |

## Open questions
See the end of PLAYER.md. Also: final product name, TorBox branding permission, keychain crate choice (`keyring`), Windows/Linux signing.

- 2026-11-04: Anime4K shaders (MIT, bloc97) are embedded and applied only by preset name through `player_filter` (UI never passes paths to mpv; `glsl-shaders` is not in SETTABLE). Motion smoothing uses built-in mpv interpolation; real SVP/RIFE integration deferred (needs VapourSynth, not store-friendly).

- 2026-11-04: macOS fullscreen uses tao simple fullscreen (same Space), not native fullscreen: native moves the window to its own Space and mpv's child window ends up above the transparent webview, hiding the controls. Logs are in-memory only (no file, no network) and URLs are reduced to scheme+host before storing.
