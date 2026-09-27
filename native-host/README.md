# XSlide — Native Windows host

Thin Win32 desktop shell that **does not use Electron**. It spawns the existing Rust backend (`xslide.exe`), waits for `GET /api/health`, and loads the product UI in a **WebView2** window.

## Prerequisites

- Rust toolchain (MSVC)
- Microsoft Edge **WebView2** Runtime
- Backend: `cargo build --release` from `xslide/`

## Build / Run

From `xslide/`:

```bat
npm run native:build
npm run native
```

Binary: `native-host/target/release/xslide-native.exe`
Default port: **8789**

Headless:

```bat
set XSLIDE_NATIVE_HEADLESS_SECS=20
native-host\target\release\xslide-native.exe --headless
```

## Packaging

```bat
npm run dist
```

`dist/XSlide_v*_win.zip`. macOS/Linux via CI (`.github/workflows/native.yml`).
