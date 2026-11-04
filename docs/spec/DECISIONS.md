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

## Open questions
See the end of PLAYER.md. Also: final product name, TorBox branding permission, keychain crate choice (`keyring`), Windows/Linux signing.
