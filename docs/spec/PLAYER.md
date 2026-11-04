# PLAYER (built-in, mpv backend, IINA-style shortcuts)

Source: planning pass on 2026-11-04. Version numbers and licence details were **not** verified; re-check before depending on them.

## Requirement
Play playlist entries (HTTPS TorBox links, large mkv/mp4, HTTP range seeking) inside the app. Backend = mpv. Frontend shortcuts copy IINA's defaults (clean-room, IINA is GPL-3.0).

## Phases
| Phase | Scope | Status |
|---|---|---|
| 0 | Spike: libmpv embedding in Tauri on macOS | TODO |
| 1 | Desktop: spawn mpv in its own window, JSON IPC control, generated `input.conf` (IINA keys), resume positions, `Play in app` button | code written, **UNVERIFIED at runtime** |
| 2 | Embedded libmpv on macOS (render API into a native child view) + HTML control bar, full key map | TODO |
| 3 | Windows + Linux embedding, packaging, signing/notarization, LGPL libmpv build pipeline | TODO |
| 4 | iOS (MPVKit or self-built LGPL framework) + Android (Kotlin plugin + libmpv.so); fallback: system players for MP4, external app for MKV | TODO |
| 5 | Polish, addon subtitles, casting | TODO |

Embedding notes: avoid a transparent-webview overlay at first (WebKitGTK and WebView2 transparency are unreliable). Use video surface plus a separate non-transparent control webview. Existing plugins (`tauri-plugin-libmpv` 0.3.2 claims Windows only; `tauri-plugin-mpv` needs mpv on PATH) are not relied upon; prefer the `libmpv2` bindings.

## Phase 1 design (implemented)
- `core/src/player.rs`: `PlayerCmd`, `bindings()`, `input_conf()`, `ipc_line`, `parse_event`, `ResumeStore` (key = FNV hash of URL without query, since TorBox links carry expiring tokens; positions <10 s or >=95% are cleared).
- `src-tauri/src/player.rs`: finds mpv (PATH plus /opt/homebrew/bin, /usr/local/bin, etc.), writes `player/queue.m3u8` and `player/input.conf` in the app data dir, spawns mpv with `--input-ipc-server`, `--playlist`, `--playlist-start`, cache and reconnect flags; reader thread observes `time-pos`, `path`, `duration`, `pause`; seeks to the saved position on `file-loaded`; emits `player://state` and `player://closed`.
- Windows uses a named pipe path, unix a socket in the temp dir (short path because of the ~104 byte limit).

## IINA key map (implemented in `bindings()`; `Meta` = Cmd)
| Action | Keys | mpv command |
|---|---|---|
| Play/pause | Space | cycle pause |
| Seek 5 s / 60 s | Left/Right, Shift+Left/Right | seek +-5 / +-60 |
| Volume | Up/Down | add volume +-5 |
| Mute | m | cycle mute |
| Speed | `[` `]` Backspace(reset) | multiply speed 0.9091 / 1.1; set speed 1 |
| Fullscreen | f, Enter | cycle fullscreen |
| Frame step | `.` `,` | frame-step / frame-back-step |
| A-B loop | l | ab-loop |
| Subtitle / audio track | s / Shift+a | cycle sub / cycle audio |
| Sub delay | z / x | add sub-delay -0.1 / +0.1 |
| Audio delay | Ctrl+= / Ctrl+- | add audio-delay +-0.1 |
| Screenshot | Ctrl+s, Cmd+s | screenshot |
| Next/prev item | `>` `<`, PgDn/PgUp | playlist-next / playlist-prev |
| Chapter | Cmd+Right/Left | add chapter +-1 |
| Stats | i | script-binding stats/display-stats-toggle |
| Close | q, Cmd+w | quit |

Verify against IINA Preferences > Key Bindings. Touch gestures for mobile: tap show/hide controls, double-tap sides seek 10 s, horizontal swipe scrub, edge swipes volume/brightness, long-press 2x, pinch fit/fill.

## Licensing and store rules
- Ship **LGPL** libmpv (`-Dgpl=false`) with LGPL-only FFmpeg (no `--enable-gpl`, `--enable-nonfree`, x264/x265/fdk-aac), **dynamically linked**, relinkable, with source offer and notices screen.
- Never ship Homebrew mpv/libmpv (GPL deps). MPVKit as published may be LGPL-3.0 with anti-tivoization terms that conflict with App Store; build our own and get legal advice before submitting.
- Google Play: dynamic `libmpv.so` fine; check 16 KB page-size requirement.
- HEVC/AAC patent exposure: prefer OS hardware decoders; get counsel before store submission.
- Phase 1 uses the user's own installed mpv (no redistribution), which avoids these issues for now.

## Link expiry and resume (TODO)
On mpv `end-file` error or HTTP 403/410: re-request the TorBox download link (needs torrent/file id on `Entry`), `loadfile <new> replace` at the saved position, retry once.

## Testing
- Unit: key map uniqueness, input.conf, IPC encode/parse, resume store (done).
- Integration (partly DONE: `core/tests/mpv_ipc.rs` covers key map + IPC against a synthetic lavfi source; range-request check still TODO): spawns `mpv --idle --vo=null --ao=null --input-ipc-server=...` against a local range-capable HTTP server (sample generated with `ffmpeg -f lavfi`), asserts loadfile, seek produces a `Range:` request, pause, end-file. Skip if mpv missing.
- Manual per OS: 20 GB mkv seek to middle, every shortcut, fullscreen, track switching, expired link, HiDPI, multi-monitor.

## Open questions
1. Is "inside the app" a hard requirement on desktop, or is the Phase 1 external mpv window acceptable for a first release?
2. OK to bundle a self-built LGPL libmpv and ship source offers?
3. Apple Developer / Google Play accounts for signing?
4. System player acceptable on mobile for MP4 meanwhile?
5. HEVC/Dolby/DTS required?
6. Floating overlay controls a must-have, or a bar beside the video?
