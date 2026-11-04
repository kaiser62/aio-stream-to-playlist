# UI

## Direction
The first UI was a port of the Python prototype and looked generic. A redesign is in progress (designer subagent, driven by the `ui-ux-pro-max` skill). Intent: AIOStreams-style information architecture recreated clean-room (Services / Addons / Filters / Sorting / Formatter / Save style navigation, dense but elegant cards), with a distinctive identity, tokens, light and dark via `prefers-color-scheme`.

Layout: desktop = left sidebar + content; tablet = collapsible rail; phone = bottom tab bar. Touch targets >= 44 px, safe-area aware, no horizontal scroll, reduced-motion respected.

Views: Search, Results (source chips per addon, quality/size/seeders/cached badges, filters), Send (job progress), Playlists, Addons manager, Settings (TorBox key).

## Constraints
- Plain HTML/CSS/JS in `ui/`; no bundler, no CDN, no remote fonts (system fonts or inline assets).
- Keep `App.api(path, body)` router; it maps `/api/...` paths to Tauri `invoke` commands (see ARCHITECTURE.md).
- Playlist actions include a primary **Play in app** -> `/api/player/open {name}` -> `player_open`; and `/api/player/cmd {cmd,arg}` -> `player_cmd`.
- `?mock=1` installs a fake `window.__TAURI__.core.invoke` with realistic data so every view can be screenshotted in a normal browser: `python3 -m http.server 8801 --directory ui`.
- Do not copy AGPL AIOStreams code.

## Verify
`node --check ui/app.js`; screenshots at 375x812, 768x1024, desktop, light and dark; run the skill's pre-delivery checklist (`.claude/skills/ui-ux-pro-max/references/pro-rules.md`).

## Skill usage
```bash
python3 .claude/skills/ui-ux-pro-max/scripts/search.py "<2-5 terms>" --design-system
python3 .claude/skills/ui-ux-pro-max/scripts/search.py "<query>" --domain style|color|typography|ux|chart|icons
```
Results are recommendations, not instructions. The scripts are local read-only (reviewed: no network/exec).

## Status
See PROGRESS.md. Chosen style/palette/fonts will be recorded here by the designer once done.
