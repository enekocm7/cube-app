---
title: Your data
description: Where times.json and cube-app.toml live, how cube-tui sharing works, and how backups keep you safe.
section: Reference
order: 2
---

## Files

| What | File | Shared with cube-tui |
|---|---|---|
| Solves and sessions | `times.json` | Yes |
| This app's settings | `cube-app.toml` | No (cube-tui uses `config.toml`) |
| Practice goals | `goals.json` | No |
| Algorithm trainer | `trainer.json` | No |
| Algorithm library | `library.json` | No |
| Multi-Blind attempts | `multi-blind.json` | No |
| Tags and solutions of solves | `solve-extras.json` | No |
| Themes | `themes/` | Yes |
| Automatic backups | `backups/` | No |

All of them live in cube-tui's data folder:

| System | Folder |
|---|---|
| Windows | `%APPDATA%\cube-tui` |
| Linux | `~/.local/share/cube-tui` |
| macOS | `~/Library/Application Support/cube-tui` |

Debug builds use a `debug` subfolder. Setting `CUBE_DATA_DIR` replaces the location entirely.

## Sharing history with cube-tui

cube is the graphical sibling of [cube-tui](https://github.com/enekocm7/cube-tui). Both apps read and write the same `times.json`, so you can move between terminal and desktop without importing anything.

- cube merges what cube-tui saved and tells you when it does.
- A solve only leaves your history when you delete it in cube — close cube before deleting one in cube-tui, or the merge will bring it back.
- Never rename or remove fields from `times.json` by hand; new optional fields are always backwards-compatible.

> Running both apps at once is supported, but whoever saves last wins per file. Let one app finish saving before editing in the other.

## Backups

- Every save keeps automatic backups in `backups/`.
- **Settings → Data** can add a folder of your own (a synced or external drive). It gets the same automatic backups plus a **Back up** button for copies that are never rotated away.
- To restore: close both apps and copy a backup over `times.json`.

If `times.json` exists but can't be read, cube says so and turns saving **off** — the file is never overwritten with an empty history.

## Multi-Blind and memo times

Multi-Blind attempts have a result in points and several scrambles, which `times.json` has no place for, so they live in `multi-blind.json`. If that file exists but can't be read, cube says so, turns Multi-Blind saving off and leaves it alone.

The memo time of a blindfolded solve is kept in `solve-extras.json` with the tags. The solve itself, with its total time, is in `times.json` as usual and cube-tui reads it (the blindfolded events are the same ones cube-tui has).

## Tags

Tags, memo times and typed solutions live in `solve-extras.json` because cube-tui would drop them from `times.json` when it saves. Each entry is tied to a solve as it was first recorded, so they survive edits and merges from cube-tui, and a solve restored from a backup gets them back. A solve that goes missing from `times.json` keeps its entry in case it returns; an entry only goes when you delete the solve in cube.

Backups and csTimer exports hold `times.json`'s data only, so copy `solve-extras.json` yourself if you want a copy of your tags and solutions.

If the file exists but can't be read, cube says so, turns tags off and leaves it alone. Your solves keep working.

## Import and export

Open **Settings → Data**:

- **Import** adds a csTimer export's sessions to your history, inferring each solve's event from its scramble. Unreadable solves are skipped and counted in the notification.
- **Export** writes every session in csTimer format for other tools.
