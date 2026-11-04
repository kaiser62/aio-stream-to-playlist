---
description: Build and test everything that can be checked locally
---
1. `source build/env.sh`
2. `cargo test -p companion-core`
3. `cargo check -p companion`
4. `node --check ui/app.js`
5. If `ui/` changed: `python3 -m http.server 8801 --directory ui`, open `http://localhost:8801/?mock=1` at 375x812, 768x1024 and desktop width, light and dark; stop the server.
6. Report exact results; list anything you could not run.
