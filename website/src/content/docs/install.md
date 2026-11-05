---
title: Install cube
description: Download cube for Windows, Linux or macOS, or build it from source with Rust.
section: Start here
order: 1
---

## Pick your build

Download the latest release from [GitHub Releases](https://github.com/enekocm7/cube-app/releases/latest):

| System | Download | How to run it |
|---|---|---|
| Windows x64 | Installer, or portable `.zip` | Run the installer and open cube from the Start menu; or unzip the portable build and run `cube.exe` |
| Linux x64 | `linux-x64` `.tar.gz` | Unpack and run `cube` |
| macOS (Apple silicon) | `macos-arm64` `.tar.gz` | Unpack and run `cube` |

> Linux and macOS builds exist but have not been fully tested yet (see [FAQ](/cube-app/docs/faq/)). If the app misbehaves there, please [open an issue](https://github.com/enekocm7/cube-app/issues) with your OS version.

## Windows notes

- The installer and portable zip are **unsigned**, so Windows SmartScreen may warn you. Click *More info → Run anyway* if you trust the release.
- No admin rights are needed for the portable build.
- Your data lives outside the install folder, so updating never wipes your solves (see [Data](/cube-app/docs/data/)).

## Build from source

You need [Rust](https://rustup.rs/) (stable). Then from the repository root:

```sh
cargo run --release
```

On Linux you also need GPUI's system libraries. The package list for Ubuntu 24.04 lives in the [release workflow](https://github.com/enekocm7/cube-app/blob/master/.github/workflows/release.yml) and comes from the [GPUI Kit install guide](https://gpui-kit.com/docs/).

Commands for working on the code:

```sh
cargo test --workspace
cargo check --workspace --all-targets
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

## First minute in the app

1. Press and **hold `Space`** until the timer turns green, then release to start.
2. Press any key to stop. Your solve appears in the list on the left.
3. Press `N` for a new scramble and keep going.
4. Press `F1` any time to see every shortcut.
