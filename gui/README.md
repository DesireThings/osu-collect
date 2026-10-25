# osu!collect GUI

Desktop window for osu!collect: Tauri 2 with a Svelte 5 frontend. The Rust side
(`src-tauri`) is a thin layer over `osu_collect::engine`, and settings live in the
same `config.toml` as the terminal version.

Screens so far:

1. **Set up**: pick osu!lazer or stable (install folder found automatically, or
   chosen), and the download folder.
2. **Add collections**: paste osu!collector links to see each collection's name,
   uploader and size. Downloading from here is not built yet.
3. **Import into osu!**: builds the one-pass lazer import folder from every
   download, with the wizard steps, a copy button and "Show folder".

## Run

Needs Rust, Node and pnpm, plus the Tauri system libraries (on Arch:
`webkit2gtk-4.1`, `libsoup3`, `gtk3`, `librsvg`).

```bash
cd gui
pnpm install
pnpm tauri dev
```

`pnpm tauri build` makes a release AppImage under `target/release/bundle/`.

`pnpm check` type-checks the frontend.
