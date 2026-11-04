# AGENTS.md: start here (Antigravity, Claude Code, Codex, any coding agent)

**Project:** *Companion for TorBox* (working name, unofficial, not affiliated with TorBox or Stremio).
A native cross-platform app (macOS, Windows, Linux, iOS, Android) that:
1. talks to **any Stremio-compatible addon** (AIOStreams, Torrentio, Cinemeta, ...) the user adds,
2. filters/dedupes the streams they return,
3. sends the chosen magnets to the user's **TorBox** account,
4. builds playable **.m3u8 playlists** and plays them, either in an external player (IINA/VLC/mpv) or in the app's own **mpv-based player** (IINA-style shortcuts).

Goal: open source (MIT), later published to Google Play and the App Store.

## Read these first (in order)
| File | What it gives you |
|---|---|
| `docs/spec/PROGRESS.md` | **Live status.** What is done / doing / next. Update it as you work. |
| `docs/spec/PRODUCT.md` | Why this exists, users, scope, non-goals, store-review posture |
| `docs/spec/ARCHITECTURE.md` | Repo layout, crates, commands/events, data flow |
| `docs/spec/PLAYER.md` | Built-in mpv player spec, phases, IINA key map, licensing |
| `docs/spec/UI.md` | Design direction and UI contract with the Rust side |
| `docs/spec/DECISIONS.md` | Decisions already made (do not relitigate) and open questions |
| `docs/spec/BUILD-AND-CI.md` | How to build/test locally and in GitHub Actions |

## Hard rules
- **Keep `docs/spec/PROGRESS.md` current**: before you stop, mark items done/doing, add what you learned, list the next step. Never claim something works unless you ran it; mark unverified items `UNVERIFIED`.
- Rust is the source of truth for logic. `core/` is pure Rust (no Tauri, no UI) and must stay unit-testable: `cargo test -p companion-core`.
- UI is plain HTML/CSS/JS in `ui/` (no bundler, no CDN, no remote fonts). It talks to Rust only through Tauri `invoke` via the `App.api(path, body)` router in `ui/app.js`. Do not rename Tauri commands without updating the router.
- `legacy/` is the old Python prototype. Do not modify it.
- Never log or commit secrets: TorBox key and addon manifest URLs (they can embed credentials). `config.json`, `playlists/`, `build/` outputs are git-ignored.
- Everything build-related lives in `build/` (local toolchain contained there). Use `source build/env.sh`. Do not install global toolchains.
- No telemetry, no analytics. No pre-installed addons except Cinemeta (store-review posture, see PRODUCT.md).
- Licensing: our code is MIT. Do not copy code from IINA (GPL-3.0) or AIOStreams (AGPL-3.0); behavior and layout may be recreated clean-room. Do not link GPL builds of mpv/FFmpeg into shipped binaries (see PLAYER.md).
- Third-party skills live in `.claude/skills/` (mirrored for Antigravity in `.agent/skills/`). For any UI/UX work, use the `ui-ux-pro-max` skill.

## Commands
```bash
source build/env.sh                 # contain rustup/cargo/target in build/
build/build.sh test                 # core unit + wiremock e2e tests
build/build.sh dev                  # run the desktop app with hot reload
build/build.sh app                  # release bundle -> build/dist (.app/.dmg on macOS)
cargo check -p companion            # fast type check of the Tauri shell
node --check ui/app.js              # JS syntax check
```
CI: `.github/workflows/native.yml` (core tests on 3 OSes + desktop builds). Remote: `github.com/kaiser62/aio-stream-to-playlist` (private).

## Antigravity notes
- This file is read as the workspace rule. Extra rules are in `.agent/rules/`, reusable workflows in `.agent/workflows/` (`/verify`, `/update-progress`).
- Prefer Planning mode for anything touching `src-tauri/` or `core/`; show a plan before editing.
- When you finish a task, run `/verify` and then `/update-progress`.
