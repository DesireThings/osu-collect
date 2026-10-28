# Contributing

osu-collect is a one-person hobby project, and help is welcome: bug reports, ideas, docs fixes and code. This page says where things go and what a good change looks like.

## Where things go

| I want to… | Go here |
|---|---|
| Ask how something works | [Discussions, Q&A](https://github.com/DesireThings/osu-shelf/discussions/categories/q-a), or [Discord](https://discord.gg/qTFvdSM2f7) for a quick answer |
| Report a bug | [A bug report](https://github.com/DesireThings/osu-shelf/issues/new/choose) |
| Suggest a feature | [A feature request](https://github.com/DesireThings/osu-shelf/issues/new/choose), or [Discussions, Ideas](https://github.com/DesireThings/osu-shelf/discussions/categories/ideas) if it's still half-formed |
| Report a security problem | [The private report form](https://github.com/DesireThings/osu-shelf/security/advisories/new), never a public issue (see [SECURITY.md](SECURITY.md)) |
| Report someone's behaviour | A private ticket on Discord (see the [Code of Conduct](CODE_OF_CONDUCT.md)) |

Issues are for bugs and concrete feature requests. Questions go in Discussions or on Discord, so the issue list stays useful.

## Before you write code

For anything bigger than a small fix, say what you'd like to do first, in an issue or on Discord. It saves you building something that doesn't fit, and it's a quick answer either way.

## Making a change

The repository is a Rust workspace (the terminal app and the shared engine at the root, the desktop app in `gui/src-tauri`, and the `osu-downloader` library) with a Svelte 5 + TypeScript frontend in `gui/`. The wiki lives in `wiki/` and is published from `main`.

Run these before you open a pull request. They're what CI runs:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo test -p osu-collect-gui --all-features
cargo test --manifest-path osu-downloader/Cargo.toml --all-features
```

If you touched `gui/`, also run `pnpm check` and `pnpm build` inside it. The Rust version is pinned in `rust-toolchain.toml`, and the lowest supported one is the `rust-version` in `Cargo.toml`. A build needs CMake and a C++17 compiler too (see the [Installation page](https://github.com/DesireThings/osu-shelf/wiki/Installation)).

A good pull request:

- does one thing, so it's quick to review and easy to undo;
- explains why, not only what;
- adds or updates a test when it fixes a bug or adds behaviour;
- updates the docs (README or `wiki/`) when it changes something people see.

The branch `main` is protected: pull requests need the `fmt`, `build-linux`, `build-windows` and `build-macos` checks to pass and are merged with a merge commit, not squashed.

## Licence

osu-collect is licensed under MIT OR Apache-2.0. Unless you say otherwise, anything you contribute is licensed the same way, with no extra terms.

## Who's who

The project is run by DesireThings. On Discord that's `desire_things` (nickname Remi). This is so you know who's who, not a way to contact anyone: bugs go in issues, questions in Discussions or Discord, and conduct reports in a ticket.

## Conduct

Everyone taking part, here and in the Discord server, follows the [Code of Conduct](CODE_OF_CONDUCT.md).
