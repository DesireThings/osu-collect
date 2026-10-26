<div align="center">

<h1>osu-collect</h1>

<p style="font-size: 22px" ><b>An osu!collector collection downloader for your terminal and desktop</b></p>

[![Build](https://img.shields.io/github/actions/workflow/status/DesireThings/osu-collect/release.yml?style=for-the-badge&logo=githubactions&logoColor=white&label=build&color=ff66aa)](https://github.com/DesireThings/osu-collect/actions/workflows/release.yml)
[![Latest release](https://img.shields.io/github/v/release/DesireThings/osu-collect?style=for-the-badge&logo=github&color=ff66aa)](https://github.com/DesireThings/osu-collect/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/DesireThings/osu-collect/total?style=for-the-badge&logo=github&color=ff66aa)](https://github.com/DesireThings/osu-collect/releases)
![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-ff66aa?style=for-the-badge)

[Features](#features) · [Install](#installation) · [Usage](#usage) · [Import](#importing-into-osu) · [Configuration](#configuration) · [FAQ](#faq) · [Wiki](https://github.com/DesireThings/osu-collect/wiki)

</div>

osu-collect is a terminal app that **downloads osu! beatmap collections from [osu!collector](https://osucollector.com)**. Paste a collection link, pick a folder, and every map batch-downloads using multiple mirrors in rotation with automatic failover. You also get a ready-to-import `collection.db` and ability to update the collections later.

> [!NOTE]
> This is a personal fork of [uwuclxdy/osu-collect](https://github.com/uwuclxdy/osu-collect) by DesireThings, and it has been changed from the original: it adds a desktop window and a one-pass osu!lazer import, and it has no osu! login. Report problems with this fork [here](https://github.com/DesireThings/osu-collect/issues), not upstream. Both projects are licensed MIT OR Apache-2.0; see [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE). Each release also has a `THIRD-PARTY-LICENSES.txt` with the licences of everything built into the binary.
>
> osu-collect is an unofficial community tool. It isn't made, endorsed or supported by ppy (the makers of osu!) or by osu!Collector; their names are used only to say what this tool works with.

<div align="center">

<img src="media/osu-collect-home.png" alt="osu-collect: paste an osu!collector collection, pick mirrors and a download folder" width="49%">
<img src="media/osu-collect-downloading.png" alt="osu-collect downloading a collection with live per-map progress, speed and ETA" width="49%">

</div>

## Features

- **No account needed:** paste a collection link and go.
- **Nine mirrors with automatic failover:** osu-collect keeps downloading even when a few mirrors are throttled. See [Mechanics](https://github.com/DesireThings/osu-collect/wiki/Mirrors).
- **Search for maps without a collection:** per-diff criteria, auto-routed between the osu! api and nzbasic's community database. See [Criteria](https://github.com/DesireThings/osu-collect/wiki/Finding-Maps).
- **A real osu! collection, verified:** integrity-checked `collection.db`, ready to import directly into your preferred osu! client. See [Verification](https://github.com/DesireThings/osu-collect/wiki/Usage).
- **Keep collections up to date:** re-check a downloaded collection later, download only new or missing beatmaps. See [Guide](https://github.com/DesireThings/osu-collect/wiki/Updating-Collections).

## Installation

### One-line install (recommended)

**Linux x64 / macOS Apple Silicon**:

```bash
curl -fsSL https://raw.githubusercontent.com/DesireThings/osu-collect/main/install.sh | bash
```

**Windows x64 (PowerShell)**:

```powershell
iwr https://raw.githubusercontent.com/DesireThings/osu-collect/main/install.bat -OutFile "$env:TEMP\osu-install.bat"; & "$env:TEMP\osu-install.bat"
```

Installs to `%LOCALAPPDATA%\Programs\osu-collect`, adds it to `PATH`, creates a desktop shortcut and registers in **Settings -> Apps -> Installed apps**. No admin needed.

### Prebuilt binary

Download from [Releases](https://github.com/DesireThings/osu-collect/releases/latest) and run it in a terminal.

> [!NOTE]
> osu-collect runs in a terminal. Windows users should be able to open it with a double click as well, but it's not guaranteed. Open an [Issue](https://github.com/DesireThings/osu-collect/issues/new/choose) if it doesn't work. Windows Terminal or PowerShell 7+ are recommended.

### Install latest from source (Rust 1.85+)

```bash
cargo install --git https://github.com/DesireThings/osu-collect
```

Building from source and more: [Installation](https://github.com/DesireThings/osu-collect/wiki/Installation) in the wiki.

## Uninstall

**Windows**: Settings -> Apps -> Installed apps -> **osu-collect** -> Uninstall.

**Linux/macOS**: run `rm ~/.local/bin/osu-collect`.

## Usage

```bash
osu-collect
```

Paste a collection link, pick a directory, press <kbd>↵</kbd>. The get maps tab has two more sources: **find** searches maps by per-difficulty criteria with no collection needed; **update** re-checks collections you downloaded and fetches only what's missing. Press <kbd>?</kbd> in-app for every key.

Full guides in the wiki:

- [Usage](https://github.com/DesireThings/osu-collect/wiki/Usage)
- [Finding maps](https://github.com/DesireThings/osu-collect/wiki/Finding-Maps)
- [Updating collections](https://github.com/DesireThings/osu-collect/wiki/Updating-Collections)
- [Keybindings](https://github.com/DesireThings/osu-collect/wiki/Keybindings)

## Importing into osu!

<details open>
<summary><b>osu! lazer</b></summary>

1. Import all downloaded maps into lazer.
2. Click `Run first time setup`, then `Next` until the **Import screen**.
3. Set `previous osu! install` to the **folder of the collection** you downloaded.
4. Click `Import content from previous version`.
5. Done. Both the maps and the collection are imported.

</details>

<details>
<summary><b>osu! stable</b></summary>

Drag the downloaded `.osz` files into osu!, then merge the generated `collection.db` with a tool like [Collection Manager](https://github.com/Piotrekol/CollectionManager). If you have no existing collections, you can just replace your `collection.db` with the generated one.

</details>

More detail: [Importing into osu!](https://github.com/DesireThings/osu-collect/wiki/Importing-into-osu) in the wiki.

## Configuration

osu-collect reads an optional config file; most settings are editable live on the config tab, where changes apply and save immediately.

| OS | Path |
|---|---|
| Linux / macOS | `~/.config/osu-collect/config.toml` |
| Windows | `%APPDATA%\osu-collect\config.toml` |

See [config.toml.example](config.toml.example) and [Configuration](https://github.com/DesireThings/osu-collect/wiki/Configuration) (wiki).

## Alternatives

| Tool | How osu-collect differs |
|---|---|
| [osu!Collector desktop client](https://osucollector.com/app) | osu!Collector's own app, which downloads from its own servers and imports straight into osu!. Its subscription pays for the site that hosts every collection this tool reads; if you use osu!Collector a lot, consider [supporting it](https://osucollector.com/app). |
| [BatchBeatmapDownloader](https://github.com/nzbasic/batch-beatmap-downloader) | Downloads by filters and criteria rather than osu!collector collections. |
| [osu-collector-dl](https://github.com/roogue/osu-collector-dl) | A CLI script with no TUI, no `collection.db` generation, and no updater. |
| [OsuCollectionDownloader](https://github.com/waylaa/OsuCollectionDownloader) | Generates `.osdb` files and needs the .NET runtime. |
| [Collection Manager](https://github.com/Piotrekol/CollectionManager) | Manages and merges existing collections; pairs well with osu-collect for stable imports. |

## FAQ

**How do I download an osu!collector collection?**
Run osu-collect, paste the collection URL, press <kbd>↵</kbd>. The collection's details come from [osu!Collector](https://osucollector.com); the maps come from public beatmap mirrors.

**Does it work with osu! lazer?**
Yes. See [Importing into osu!](#importing-into-osu). The generated `collection.db` imports through lazer's "first-time-setup".

**Do I need an osu! account?**
Not for downloading collections. osu-collect never asks for your osu! login. Searching for maps through the osu! API (the default for find) needs an API app of your own, which you make on your osu! account page; see [Finding Maps](https://github.com/DesireThings/osu-collect/wiki/Finding-Maps#your-osu-api-app).

## Documentation

Full reference: [osu-collect wiki](https://github.com/DesireThings/osu-collect/wiki).

## What's next

- [ ] All features of [BatchBeatmapDownloader](https://github.com/nzbasic/batch-beatmap-downloader) (🚧 in the works)
- [ ] Integrate osu!Stats

## Acknowledgments

- [osu-downloader](osu-downloader/): bundled Rust library handling downloads
- [osu-db](https://crates.io/crates/osu-db): reading the osu!stable db
- [ratatui](https://ratatui.rs): TUI

Inspired by [BatchBeatmapDownloader](https://github.com/nzbasic/batch-beatmap-downloader).

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option. Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this work shall be dual licensed as above, without any additional terms or conditions.
