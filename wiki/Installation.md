# Installation

> [!NOTE]
> This fork hasn't published a release yet, so the installers, prebuilt binaries and self-updating below have nothing to fetch. Until the first release, [install from source](#install-from-source).

## One-line install (recommended)

**Linux x64 / macOS Apple Silicon**:

```bash
curl -fsSL https://raw.githubusercontent.com/DesireThings/osu-collect/main/install.sh | bash
```

**Windows x64 (PowerShell)**:

```powershell
iwr https://raw.githubusercontent.com/DesireThings/osu-collect/main/install.bat -OutFile "$env:TEMP\osu-install.bat"; & "$env:TEMP\osu-install.bat"
```

The Windows installer needs no admin rights. It does the following:

- installs to `%LOCALAPPDATA%\Programs\osu-collect`
- adds the install folder to your `PATH`
- creates a desktop shortcut
- registers an uninstall entry in **Settings → Apps → Installed apps**

## Prebuilt binary

Download the binary for your OS from [Releases](https://github.com/DesireThings/osu-collect/releases/latest) and run it in a terminal.

osu-collect runs in a terminal. On Windows a double click usually opens it too, though that's not guaranteed; Windows Terminal or PowerShell 7+ are recommended. If it doesn't start, open an [issue](https://github.com/DesireThings/osu-collect/issues/new/choose).

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

## Desktop app from source

The desktop app has no packaged build yet. To run it from a clone you also need Node.js and pnpm, plus on Linux the WebKitGTK 4.1 development package (`webkit2gtk-4.1` on Arch, `libwebkit2gtk-4.1-dev` on Debian/Ubuntu; see [Tauri's prerequisites](https://v2.tauri.app/start/prerequisites/) for the full list):

```bash
git clone --recurse-submodules https://github.com/DesireThings/osu-collect
cd osu-collect/gui
pnpm install
pnpm tauri dev
```

It's been run on Linux only so far. See [Desktop app](Desktop-App) for how to use it.

## Self-updating

osu-collect checks GitHub for a newer release on launch. With `update.auto_update = true` (the default) it downloads the release, verifies its checksum against the published one, then replaces itself. With `auto_update = false` it still checks and only notifies (toast + header indicator); press <kbd>u</kbd> to read the changelog and apply the update when you want. Opt into prerelease builds with `update.prereleases = true`. See [Configuration](Configuration#update).

## Uninstall

- **Windows**: Settings → Apps → Installed apps → **osu-collect** → Uninstall.
- **Linux / macOS**: `rm ~/.local/bin/osu-collect`

Settings live in a separate config folder, run history in a data folder; delete both for a full cleanup (paths in [Configuration](Configuration#other-files)).

## Building from source (development)

```bash
cargo build --release
```

`build.sh` cross-builds Linux + Windows binaries into `build/`. The bundled `osu-downloader` library is a path dependency, not a workspace member; test it separately with `cargo test --manifest-path osu-downloader/Cargo.toml --all-features`.
