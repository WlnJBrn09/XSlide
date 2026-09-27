# XSlide

Local-first presentation app with liquid-glass UI and a **Rust** backend.

## Desktop app (native)

No Electron. WebView2 / WebKit host + local Rust backend.

```bash
npm run native:build
npm run native
npm run dist
```

Port **8789**. Package: `dist/XSlide_v*_win.zip`

## Dev server

```bash
cargo run
```

http://127.0.0.1:8789

## Files

Open PPTX, PDF, images, and XSlide JSON from the Documents library or file picker. Export an editable JSON copy, text, or PPTX from the header. PPTX export includes slide order, positioned text, text styling, backgrounds, and embedded images. Audio and video slides report an export error; keep a JSON copy for those presentations.

## License

MIT
