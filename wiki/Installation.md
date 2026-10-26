# Installation

osu-collect comes as a **desktop app** (a window with buttons) and a **terminal app**. Both use the same settings, so you can have either or both. Downloads are on the [latest release](https://github.com/DesireThings/osu-collect/releases/latest).

> [!NOTE]
> osu-collect is in early access (version 0.x). The Windows and macOS desktop installers are built and checked automatically but haven't been run on a real Windows PC or Mac yet. If one doesn't work, please [open an issue](https://github.com/DesireThings/osu-collect/issues/new).

## Desktop app

The apps aren't signed with a paid certificate, so Windows and macOS warn the first time you open them.

### Windows 10/11

1. Download `osu-collect-desktop-<version>-windows-x64-setup.exe` and run it.
2. If Windows says "Windows protected your PC", click **More info → Run anyway**.

It installs for your user only (no admin prompt), to `%LOCALAPPDATA%\osu-collect`, with a Start menu shortcut (and a desktop one if you tick it at the end). It downloads Microsoft's WebView2, which draws the window, if your PC doesn't have it yet (Windows 11 and most Windows 10 PCs do).

### macOS (Apple Silicon)

1. Download `osu-collect-desktop-<version>-macos-arm64.dmg`, open it and drag **osu-collect** to Applications.
2. Open it. macOS says it can't check the app for malicious software: open **System Settings → Privacy & Security** and click **Open Anyway**.

There are no Intel Mac builds.

### Linux

**AppImage** (most distributions): download `osu-collect-desktop-<version>-linux-x64.AppImage`, then:

```bash
chmod +x osu-collect-desktop-*-linux-x64.AppImage
./osu-collect-desktop-*-linux-x64.AppImage
```

It needs FUSE, which most desktops already have. If it doesn't start, install `fuse3` (the package has that name on Arch, Debian and Ubuntu), or run it with `--appimage-extract-and-run`.

**.deb** (Debian, Ubuntu and derivatives): download `osu-collect-desktop-<version>-linux-amd64.deb`, then:

```bash
sudo apt install ./osu-collect-desktop-*-linux-amd64.deb
```

It installs `osu-collect-gui` with a menu entry, and pulls in WebKitGTK and GTK from your distribution.

### Updating

The desktop app doesn't update itself yet. Download the new version and install it over the old one (on Linux, replace the AppImage or install the new .deb); your settings stay.

## Terminal app

### One-line install (recommended)

**Linux x64 / macOS Apple Silicon**:

```bash
curl -fsSL https://raw.githubusercontent.com/DesireThings/osu-collect/main/install.sh | bash
```

It installs `osu-collect` to `~/.local/bin` (or `$XDG_BIN_HOME`).

**Windows x64 (PowerShell)**:

```powershell
iwr https://raw.githubusercontent.com/DesireThings/osu-collect/main/install.bat -OutFile "$env:TEMP\osu-install.bat"; & "$env:TEMP\osu-install.bat"; Remove-Item "$env:TEMP\osu-install.bat"
```

The Windows installer needs no admin rights. It does the following:

- installs to `%LOCALAPPDATA%\Programs\osu-collect`
- adds the install folder to your `PATH`
- creates **osu-collect (terminal)** shortcuts on the desktop and in the Start menu
- registers **osu-collect (terminal app)** in **Settings → Apps → Installed apps**

Both installers check the download against the SHA-256 GitHub records for it.

### Prebuilt binary

Download `osu-collect-linux-x64`, `osu-collect-windows-x64.exe` or `osu-collect-macos-arm64` from the [latest release](https://github.com/DesireThings/osu-collect/releases/latest) and run it in a terminal (on Linux and macOS, `chmod +x` it first).

osu-collect runs in a terminal. On Windows a double click usually opens it too, though that's not guaranteed; Windows Terminal or PowerShell 7+ are recommended. If it doesn't start, open an [issue](https://github.com/DesireThings/osu-collect/issues/new).

### Self-updating

The terminal app checks GitHub for a newer release on launch. With `update.auto_update = true` (the default) it downloads the release, checks it against the SHA-256 GitHub records for the file, then replaces itself. With `auto_update = false` it still checks and only notifies (toast + header indicator); press <kbd>u</kbd> to read the changelog and apply the update when you want. Opt into prerelease builds with `update.prereleases = true`. See [Configuration](Configuration#update).

## Checking a download by hand

Each release has a `SHA256SUMS` file. In the folder with your downloads: `sha256sum -c SHA256SUMS --ignore-missing` on Linux, `shasum -a 256 -c SHA256SUMS --ignore-missing` on macOS.

## Uninstall

- **Desktop app, Windows**: Settings → Apps → Installed apps → **osu-collect** → Uninstall.
- **Desktop app, macOS**: drag **osu-collect** from Applications to the Bin.
- **Desktop app, Linux**: delete the AppImage, or `sudo apt remove osu-collect` for the .deb.
- **Terminal app, Windows**: Settings → Apps → Installed apps → **osu-collect (terminal app)** → Uninstall.
- **Terminal app, Linux / macOS**: `rm ~/.local/bin/osu-collect`

Settings live in a separate config folder, run history in a data folder; delete both for a full cleanup (paths in [Configuration](Configuration#other-files)).

## If you also use uwuclxdy's osu-collect

This is a fork of [uwuclxdy/osu-collect](https://github.com/uwuclxdy/osu-collect), and both use the same settings folder. This version deletes a saved osu!lazer login from the original, because it no longer signs in through osu!lazer's client. Your copy of the original keeps updating from its own releases.

## Install from source

You need:

- a current stable Rust (tested with 1.99; anything older than 1.88 won't build),
- CMake and a C++17 compiler, for the bundled realm library osu-collect uses to read osu!lazer's database,
- git.

Then:

```bash
cargo install --git https://github.com/DesireThings/osu-collect
```

This installs the terminal app as `osu-collect`. The first build takes a while because it compiles realm.

### Desktop app from source

You also need Node.js and pnpm, plus on Linux the WebKitGTK 4.1 development package (`webkit2gtk-4.1` on Arch, `libwebkit2gtk-4.1-dev` on Debian/Ubuntu; see [Tauri's prerequisites](https://v2.tauri.app/start/prerequisites/) for the full list):

```bash
git clone --recurse-submodules https://github.com/DesireThings/osu-collect
cd osu-collect/gui
pnpm install
pnpm tauri dev
```

`pnpm tauri build` makes an AppImage and a .deb under `target/release/bundle/`. A build like that has no licence list under Settings → About; release builds add it with `--config src-tauri/tauri.release.conf.json` after running `scripts/third-party-licenses.sh --desktop`. See [Desktop app](Desktop-App) for how to use it.

## Building from source (development)

```bash
cargo build --release
```

The bundled `osu-downloader` library shares the workspace and its `Cargo.lock`, but `cargo test` at the root doesn't run its tests; run them with `cargo test --manifest-path osu-downloader/Cargo.toml --all-features`.
