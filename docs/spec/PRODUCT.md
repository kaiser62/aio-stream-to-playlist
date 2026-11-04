# PRODUCT

## Problem
The user has an AIOStreams (Stremio addon) setup and a TorBox account. Stremio results are noisy: duplicates, wrong or missing quality info, uncached torrents. They want to filter/correct results, push the chosen magnet to TorBox, and get a playable playlist for IINA/VLC/mpv/Infuse.

## Product
"Companion for TorBox": one native app on every platform.
- **Addon manager**: add/remove/enable/rename/reorder/test any Stremio addon by manifest URL; per-addon timeout, health, configure page; import/export list. Cinemeta is the only default (metadata/search).
- **Search -> streams**: query all enabled addons in parallel (per-addon timeout, error isolation), merge and dedupe by infoHash/url, show quality, size, seeders, cached flag, source addon.
- **Send to TorBox**: add magnet, wait until ready, pick the right file(s) (episode matching), request direct download links.
- **Playlists**: save as `.m3u8`, export/copy, open in IINA/VLC/mpv, or play in the built-in mpv player.
- **Later**: TorBox library management (browse, delete, usage/plan).

## Users
Individual power users who already run their own addons and TorBox account. Single user per device; no accounts, no server, no LAN mode.

## Non-goals
- No scraping, indexing, or bundled content sources. No pre-installed piracy-adjacent addons.
- No hosting/streaming of content by us. No telemetry.
- Not a Stremio replacement; no catalog browsing UI beyond search (for now).

## Store-review posture (biggest risk)
Apple (5.2 IP) and Google reject apps seen as facilitating piracy. Mitigations: ship only Cinemeta by default; describe the app as "TorBox companion / Stremio addon manager"; first-run notice that users are responsible for content; neutral name with "unofficial, not affiliated" notice; confirm TorBox/Stremio branding terms before submission; fallback build that hides the addon manager behind a feature flag if rejected.

## Licensing
MIT (App Store incompatible with GPL). Dependencies checked with `cargo deny` before release. See PLAYER.md for mpv/FFmpeg rules.

## Privacy
Everything stays on device. Only requests: the user's addons, the user's TorBox account, Cinemeta. Privacy policy required by both stores ("no data collected").
