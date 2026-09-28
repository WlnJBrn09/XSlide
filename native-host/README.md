# XSlide — native Linux host

Thin desktop shell that **does not use Electron**. It spawns the existing Rust backend (`xslide`), waits for `GET /api/health`, and loads the product UI in a **WebKitGTK** window via `wry`/`tao`.

## Prerequisites

- Rust toolchain
- WebKitGTK: `libwebkit2gtk-4.1-dev` (Debian/Ubuntu) or `webkit2gtk4.1-devel` (Fedora)
- Backend built once: `cargo build --release` from `xslide/`

## Build

From `xslide/`:

```sh
npm run native:build
```

Or:

```sh
cargo build --release
cargo build --release --manifest-path native-host/Cargo.toml
```

Binary: `native-host/target/release/xslide-native`

## Run

```sh
npm run native
```

Headless (starts backend + health wait, no window; useful for CI/HTTP checks):

```sh
XSLIDE_NATIVE_HEADLESS_SECS=20 native-host/target/release/xslide-native --headless
```

Default port: **see product README** (`PORT` env overrides).

## Layout

- **Dev**: host walks up from its exe path until it finds `static/index.html` and uses `target/release|debug/xslide`.
- **Packaged**: place `xslide-native` next to `backend/xslide` and `static/`.

## Packaging

From the product root:

```sh
npm run dist:deb   # Debian/Ubuntu .deb (build on Debian trixie)
npm run dist:rpm   # Fedora/RHEL/openSUSE .rpm (build with rpmbuild)
```

See [Debian packaging](../../packaging/debian/README.md) and [RPM packaging](../../packaging/rpm/README.md).
