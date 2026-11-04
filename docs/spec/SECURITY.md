# Security: how the TorBox key is protected

## What holds the key (new app)
- `config.json` (TorBox key + addon manifest URLs, which can embed credentials) is **encrypted with ChaCha20-Poly1305**.
  The 32-byte data key lives in the OS keychain (macOS Keychain, Windows Credential Manager, iOS Keychain), never beside the file.
- Where no keychain is wired (Linux, Android) the file is written atomically with mode 0600 but is **not encrypted** (known gap).
- If the file is encrypted and the keychain is unavailable the app goes read-only and never overwrites it.
- The webview never receives the key: `config_get` returns only `has_key`. Network errors have URLs stripped (`without_url`); the Logs tab redacts URLs to scheme://host.
- CSP is locked to `'self'` (no remote scripts, no inline handlers, no eval), so injected addon metadata cannot run code or call out.

## Known exposure surfaces (audit)
1. **TorBox `requestdl` needs the key as a `token` query parameter.** It is sent over TLS from Rust only; the resulting direct link does not contain the key. UNVERIFIED whether the API also accepts a bearer header for this call (would remove the query param).
2. **Addon manifest URLs often embed the key** (e.g. config-in-URL addons). They are stored encrypted, shown masked in the UI, and `addon_url` / `addons_export` return them on explicit user action only.
3. **Legacy Python app playlists contain `requestdl?token=<key>` in plain text** (e.g. in ~/Downloads). Delete or rotate.
4. Same-user malware with keychain access can still read the key; no desktop app can fully prevent that.
5. Updater payloads are verified against the public key in `tauri.conf.json`; the private key stays in `build/keys/` (git-ignored) and the `TAURI_SIGNING_PRIVATE_KEY` repo secret.

## If a key was shown anywhere (chat, logs, screenshots, URLs): rotate it in TorBox settings.
