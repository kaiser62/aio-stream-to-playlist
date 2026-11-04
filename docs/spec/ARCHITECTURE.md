# ARCHITECTURE

```
core/                    companion-core (pure Rust, no Tauri/UI)  -> cargo test -p companion-core
  src/addons.rs          manifest fetch/validate, add_addon, fetch_streams (parallel, timeouts), search, Cinemeta
  src/parse.rs           Stream model, parse_stream, dedupe, pick_file
  src/torbox.rs          TorBox API: createtorrent, mylist, requestdl
  src/jobs.rs            send job (add magnet, wait, pick file, write playlist)
  src/playlist.rs        Entry{title,url}, m3u8 build/parse, library on disk
  src/config.rs          Config{torbox_key, addons, seeded}, 0600 JSON
  src/player.rs          PlayerCmd, IINA key map, input.conf, mpv IPC encode/parse, ResumeStore
  tests/e2e.rs           wiremock: fake addons + TorBox
src-tauri/               Tauri 2 shell (identifier app.companion.streamplaylist)
  src/lib.rs             AppState + #[tauri::command]s + Builder
  src/player.rs          desktop-only mpv backend (spawn, JSON IPC, reader thread, events)
  tauri.conf.json, capabilities/default.json
ui/                      plain HTML/CSS/JS (index.html, app.css, app.js, icon.svg/png)
legacy/                  old Python prototype (frozen)
build/                   contained toolchain + scripts (setup.sh, build.sh, env.sh), outputs in build/{target,dist}
.github/workflows/       native.yml (primary), legacy-python.yml
docs/spec/               this spec kit
.claude/skills/          third-party skills (ui-ux-pro-max); mirrored in .agent/skills/
```

## Data flow
UI (`App.api`) -> `invoke(command)` -> Rust command -> `companion-core` -> HTTP to addons/TorBox. State lives in Rust `AppState{dir, cfg, jobs, client}`; app data dir holds `config.json`, `playlists/`, `resume.json`, `player/`.

## Tauri commands (contract with the UI)
config_get/config_set; addons_list/add/update/remove/reorder/refresh/seed/export/import; addon_url; addon_configure; search; fetch_streams; send; job_get; library; playlists_list; playlist_read/text/save/delete/export; open_playlist (external player); **player_open{name,index?}, player_cmd{cmd,arg?}, player_close, player_keys**.
Events: `player://state {pos,duration,path,paused}` (about 1/s), `player://closed`.

## Platform notes
- Desktop: full feature set. Mobile (iOS/Android): same UI and core; `open_playlist` and `player_*` currently return an error (use Copy/Export). Mobile playback strategy is in PLAYER.md.
- HTTP: reqwest with rustls (no OpenSSL). All network calls are in Rust, so no CORS.
- Secrets: never logged; addon URLs are masked in UI/logs. Move to OS keychain (TODO).

## Testing
Unit tests in core, wiremock e2e, JS syntax check, CI matrix on 3 OSes. UI verified in the built-in browser using `?mock=1` (mock `invoke`).
