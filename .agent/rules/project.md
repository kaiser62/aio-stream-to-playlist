---
trigger: always_on
---
# Project rules (Companion for TorBox)
- Read `AGENTS.md` and `docs/spec/PROGRESS.md` at the start of every task.
- Rust owns the logic (`core/`, `src-tauri/`); the UI (`ui/`) is plain HTML/CSS/JS calling Rust via `App.api` -> Tauri `invoke`.
- Never modify `legacy/`. Never log or commit secrets (TorBox key, addon URLs).
- Use the `ui-ux-pro-max` skill for any UI/UX work.
- Do not copy code from IINA (GPL) or AIOStreams (AGPL).
- Never claim something works unless you ran it. Mark it UNVERIFIED in PROGRESS.md otherwise.
- Before ending a task: run `/verify`, then `/update-progress`.
