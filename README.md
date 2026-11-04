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
- Solve history, with details you can edit (time, event, scramble, penalty, date and comment) and a full audit trail of every change
- Current and best single, mo3, ao5, ao12, ao50 and ao100, plus mean, deviation and DNF count
- Stats screen with charts of your times, your ao5 and their distribution
- Light and dark themes, plus cube-tui TOML themes with a color-picker editor
- Fully keyboard-controllable: every timer, solve, event, session and screen action has a rebindable shortcut, any
  key stops the timer, and `F1` lists the bindings
- Optional bundled JetBrains Mono for the timer digits, all times, or the whole interface
- Import and export csTimer sessions, with file pickers, from Settings → Data
- Checks GitHub for new releases and updates itself, verifying the download against the release's SHA-256
- Interface in English and Spanish

Features still on the way are tracked in the [issues](https://github.com/enekocm7/cube-gui/issues).

## Install

Download the installer or a portable build from the [releases](https://github.com/enekocm7/cube-gui/releases) page.
Installed builds update themselves silently; portable builds swap the executable and restart. Both wait for you to
accept an update, and the download is checked against the release's `SHA256SUMS` file before anything is installed.
Turn the automatic check off, change its frequency or release channel, or check manually from
**Settings → Updates**.

To build from source, install [Rust](https://rustup.rs/) and run:

```sh
cargo run --release
```

## How to use it

cube is designed to be fully usable from the keyboard: every action below has a shortcut, dialogs stay reachable
with `Tab`, `Enter` and `Space`, `Enter` runs a dialog's primary action while `Esc` closes it, and the **Edit** button on the `F1` shortcuts panel rebinds or removes them. See
[docs/keybinds.md](docs/keybinds.md) for the full reference, the key name syntax, and how to change bindings from
`cube-gui.toml`.

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
| `Shift + D` | Toggle DNF on the selected solve |
| `E` / `Shift + E` | Next / previous event |
| `]` / `[` | Next / previous session |
| `S` | New session |
| `F2` | Rename session |
| `Shift + S` | Delete session |
| `Ctrl + Z` | Undo the last solve or session deletion (repeatable) |
| `Ctrl + Y` | Redo an undone deletion |
| `A` | Show stats, or return to the timer |
| `Ctrl + ,` | Open settings |
| `F1` | Open the keyboard shortcuts |

Actions that need a solve use the keyboard selection, or the latest solve when nothing is
selected. Click a solve in the list to see its details or edit it.

Hold `N`, `Up` or `Down` to repeat: scrambles keep coming and navigation keeps moving until you release the key.

Dialogs size themselves to the window: settings, solve details and the theme editor get wider and taller on a large
screen, and scroll their bodies when the window is too small to show everything.

`F1` opens the shortcuts panel. Its **Edit** button (opposite **Close**) shows a pencil to rebind an action and a
danger-coloured trash can to remove its shortcut. Bindings can also be edited by hand in the `[keybindings]` table
in `cube-gui.toml`; [docs/keybinds.md](docs/keybinds.md) documents every action, the key syntax and the validation
rules.

The shortcuts panel and the settings dialog both start with a search field (reachable with `Tab`). Type to
filter rows by label or current value, such as `space`, `ctrl` or `unassigned` for shortcuts, or `dark` or `off` for
settings; matching ignores case and uses the interface language. Sections with no match are hidden.

Drag the inner edge of the solve list or the statistics panel to resize it. Neither panel gets
narrower than 180 px or wider than 350 px, and double-click an edge to restore its default width. The widths are
remembered between launches.

The start hint is only shown until you start your first solve, so the timer stays clean once you
know the controls. It names whichever key the timer is bound to.

The solve list scrollbar only appears while you scroll; turn on **Settings → Appearance → Always show the solve
list scrollbar** to keep it visible.

Times use the system's monospace font. **Settings → Appearance → Font** can switch to
[JetBrains Mono](https://github.com/JetBrains/JetBrainsMono) for the timer only, for every time, or for the whole
interface; the font is bundled under the SIL Open Font License and stays off by default.
**Settings → Appearance → Scramble size** picks Small, Normal (default), Large, Largest or Custom,
which takes an exact size in points (6 to 120). Long scrambles still scroll.

## Themes

cube reads the same theme files as [cube-tui](https://github.com/enekocm7/cube-tui) from the `themes` folder in your
data folder, so a theme works in both apps. Settings → Theme lists every valid theme: pick one to apply it, use the
pencil to edit it, the trash can to delete it, or **New theme** to create one. Themes added to the folder appear the
next time the settings dialog opens.

The editor has a color picker for each of the seven colors, a text field that can switch between Hex, RGB, RGBA, HSL
and HSLA, and a live preview. Rename a theme by changing its name; cube saves it as `<name>.toml`. Theme files store
opaque colors, so an alpha value typed into an RGBA or HSLA field is ignored when saving.

A theme is a TOML file of six-digit colors:

```toml
background = "#000000"
border = "#FFFFFF"
scramble = "#FFFFFF"
selection = "#3399FF"
selection_text = "#000000"
text = "#FFFFFF"
accent = "#FFD700"
```

`accent` is optional and defaults to gold, so older cube-tui themes keep working. cube creates `default.toml` the
first time it makes the themes folder; a deleted theme is not recreated.

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
| `crates/cube-updater` | GitHub release check, verified download and self-update |
| `crates/cube-ui` | The GPUI interface |

## License

[GPL-3.0-or-later](LICENSE).
