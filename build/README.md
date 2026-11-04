# build/

Everything needed to compile the native app locally lives here, self-contained.

```bash
build/setup.sh        # one-time: Rust (rustup) + Tauri CLI, installed into build/ only
build/build.sh test   # Rust unit + end-to-end tests
build/build.sh dev    # run the desktop app with live reload
build/build.sh app    # tests + installers -> build/dist
```

Ignored by git (large/generated): `rustup/ cargo/ target/ node/ downloads/ dist/`.
CI (GitHub Actions) builds independently with its own toolchain; see `.github/workflows/native.yml`.
iOS builds additionally need the full Xcode app; Android needs the Android SDK/NDK.
