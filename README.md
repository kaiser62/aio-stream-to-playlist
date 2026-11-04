# Stream Playlist

AIOStreams (Stremio addon) → filter/clean → TorBox → `.m3u8` playlist for IINA, VLC, mpv, Infuse.
Works in any browser (desktop, iPad, iPhone, Android) and installs as an app (Add to Home Screen / Install).

```bash
python3 app.py                          # http://127.0.0.1:8790 (this computer only)
HOST=0.0.0.0 python3 app.py             # other devices on your network (requires a PIN set in Settings)
python3 -m unittest discover -s tests   # tests
```

Standalone Windows/macOS/Linux binaries are built by GitHub Actions (see Actions → artifacts).
Config (manifest URL, TorBox key, PIN) is stored in `config.json` (or `~/.aio-stream-to-playlist/` in built binaries) and never committed.

Debug logging: `DEBUG=1 python3 app.py` (or `DEBUG=1 ./stream-playlist`) prints startup info, every request, upstream AIOStreams/TorBox calls and job errors. Tokens, keys and PINs are redacted.
