<p align="center"><img src="assets/logo.svg" width="96" alt="cube logo"></p>

<h1 align="center">cube</h1>

<p align="center">A desktop timer and session manager for speedcubing, built with Rust and <a href="https://gpui-kit.com/docs/">GPUI Kit</a>.</p>

![cube's timer screen](docs/screenshot.png)

cube is the graphical sibling of [cube-tui](https://github.com/enekocm7/cube-tui). Both apps read and write the same
solve history, so you can move between the terminal and the desktop without importing anything.

## Features

- Timer with WCA inspection that counts up to 15 seconds: `+2` and DNF are applied automatically, optional 8 s and
  12 s audio cues warn you, and hold colors work like a stackmat's
- Official WCA random-state scrambles for 12 events, generated with `tnoodle-rs`
- Sessions with custom names
- Solve history, with details, comments, penalties and edits that keep an audit trail
- Current and best single, mo3, ao5, ao12, ao50 and ao100, plus mean, deviation and DNF count
- Stats screen with charts of your times, your ao5 and their distribution
- Import and export csTimer sessions, with file pickers, from Settings → Data
- Light and dark themes, in English and Spanish

Features still on the way are tracked in the [issues](https://github.com/enekocm7/cube-gui/issues).

## Install

Download the installer or a portable build from the [releases](https://github.com/enekocm7/cube-gui/releases) page.

To build from source, install [Rust](https://rustup.rs/) and run:

```sh
cargo run --release
```

## How to use it

| Key | Action |
|---|---|
| `Space` | Start inspection, or hold and release to start the solve, and press to stop |
| Any other key | Stop a running solve |
| `Esc` | Cancel the current attempt without recording it, or leave the stats screen |
| `N` | New scramble |
| `I` | Turn WCA inspection on or off |
| `Up` / `Down` | Select a newer or older solve |
| `Enter` | Open the selected solve's details |
| `D` | Delete the selected solve |
| `2` | Toggle `+2` on the selected solve |
| `Shift+D` | Toggle DNF on the selected solve |
| `E` / `Shift+E` | Next / previous event |
| `]` / `[` | Next / previous session |
| `S` | New session |
| `F2` | Rename session |
| `Shift+S` | Delete session |
| `A` | Show stats, or return to the timer |
| `Ctrl+,` | Open settings |
| `F1` | Show the keyboard shortcuts |

Actions that need a solve use the keyboard selection, or the latest solve when nothing is
selected. Click a solve in the list to see its details or edit it.

Every binding above can be changed with a `[keybindings]` table in `cube-gui.toml` (see below
for the file's location). Anything you leave out keeps its default, and a broken entry or a
duplicate is ignored rather than breaking the app. `F1` always shows the shortcuts in effect.

```toml
[keybindings]
timer = "Space"             # must be an unmodified key
cancel = "Escape"
next_scramble = "N"
toggle_inspection = "I"
previous_solve = "Up"
next_solve = "Down"
open_solve = "Enter"
delete_solve = "D"
penalty_plus_two = "2"
penalty_dnf = "Shift+D"
next_event = "E"
previous_event = "Shift+E"
next_session = "]"
previous_session = "["
new_session = "S"
rename_session = "F2"
delete_session = "Shift+S"
open_stats = "A"
open_settings = "Ctrl+,"
show_help = "F1"
```

The start hint is only shown until you start your first solve, so the timer stays clean once you know the
controls. It names whichever key the timer is bound to.

Open **Settings → Data** to import a csTimer export or to export every session. Importing adds the
file's sessions to your history, infers each solve's event from its scramble, and keeps penalties and
comments; solves that can't be read are skipped and counted in the notification.

## Your data

Solves are stored in cube-tui's `times.json`, in your user data folder (`%APPDATA%\cube-tui` on Windows). Settings for
this app are in `cube-gui.toml` in the same folder. Debug builds use a `debug` subfolder, and the `CUBE_DATA_DIR`
environment variable overrides the location.

If `times.json` can't be read, cube says so and turns saving off, so the file is never overwritten.

Don't keep cube and cube-tui open at the same time: each one overwrites the file with what it has in memory.

## Project layout

| Crate | Contents |
|---|---|
| `cube-gui` (root) | The `cube` binary and icon embedding |
| `crates/cube-core` | Solves, statistics, the timer state machine, scrambles and persistence, with no GUI dependency |
| `crates/cube-locale` | Translated strings |
| `crates/cube-ui` | The GPUI interface |

## License

[GPL-3.0-or-later](LICENSE).
