---
title: Settings reference
description: Every option, where it lives in the dialog, and what it writes to cube-app.toml.
section: Reference
order: 1
---

Open settings with `Ctrl + ,`. They save to `cube-app.toml` next to your solves (see [Data](/cube-app/docs/data/)). The dialog lists its categories on the left and shows the selected one on the right. The search field above the list looks through every category. The [keyboard shortcuts](/cube-app/docs/keybinds/#keys-inside-dialogs) page covers moving around without the mouse.

## General

### Language

Sixteen languages, including English, Spanish, German, French, Japanese, and Chinese. Dates and counts follow the chosen language. Empty (the default) follows the system.

### Notifications and confirmations

| Option | Default | What it does |
|---|---|---|
| Notifications (toasts) | On | Merge, import, delete, and update notices |
| Confirm before deleting | On | Asks before deleting a solve or session; Undo works either way |

## Timer

| Option | Default | What it does |
|---|---|---|
| WCA inspection | On | Count down 15 s before the solve; overruns become `+2`/DNF |
| Inspection audio | Off | Beeps at 8 s and 12 s of inspection |
| Celebrate personal bests | On | Flashes the digits for a new best single or ao5 |
| Personal best sound | Off | Plays a chime for a new best single or ao5 |
| Solves before a best counts | 100 | Earlier solves needed before bests are celebrated or announced. From 0 to 100000; blank restores 100 |
| Reduce motion | Off | Holds a steady color for a personal best instead of flashing |
| Touch the digits | Off | Pressing the digits with mouse/touch works like the timer key |

### Scrambles

| Option | Default | What it does |
|---|---|---|
| Shorter 3x3x3 scrambles | Off | Searches longer for short scrambles; noticeably slower to generate |
| Show the scramble preview | On | Draw the scrambled puzzle under the timer and in solve details (`P` toggles it too) |
| 3D scramble preview | Off | Draw the puzzle in 3D, to turn around with the mouse or with `Ctrl` and the arrow keys. Clicking the preview, or `Ctrl + P`, toggles it. Saved as `preview_3d` |
| Preview size, per event (**Appearance → Sizes per event**) | Normal | Small, Normal, Big or Huge for 2x2x2, 3x3x3, 4x4x4 and Square-1; up to Big for FTO; Small or Normal for the other events. Saved in `cube-app.toml` under `[preview_sizes]` for events that differ from Normal |

## Appearance

### Theme

Light, dark, or system — plus your theme files. See [Themes](/cube-app/docs/themes/).

### Font size

| Option | Default | What it does |
|---|---|---|
| Font | All times | Cube Mono for the timer only, every time, or the whole interface, or the system fonts |
| Text size | Normal | Small / Normal / Large / Largest / Custom (8–24 pt) for the interface text |
| Timer size | Normal | Small / Normal / Large / Largest / Custom (24–240 pt) for the timer's digits |
| Timer size, per event (**Appearance → Sizes per event**) | The general size | The same sizes for one event, with its own Custom points. Saved in `cube-app.toml` under `[timer_sizes]` for events that differ from the general size |
| Scramble size | Normal | Small / Normal / Large / Largest / Custom (6–120 pt) |
| Scramble size, per event (**Appearance → Sizes per event**) | The general size | The same sizes for one event, with its own Custom points. Saved in `cube-app.toml` under `[scramble_sizes]` for events that differ from the general size |
| Library size | Normal | Small / Normal / Big for the case rows, drawings and panels in the library |

### Solve list

| Option | Default | What it does |
|---|---|---|
| ao5 in the solve list | Off | Adds a rolling ao5 column next to each solve |
| Always show the solve list scrollbar | Off | Keeps it visible instead of only while scrolling |

## Library

| Option | Default | What it does |
|---|---|---|
| 3D cases | Off | Draws the highlighted case and the case under review in 3D, to turn around with the mouse or the keyboard. Clicking a case's drawing, or `Ctrl + P` in the library, toggles it. Saved as `library_3d` |

The library's sets are grouped by puzzle, then by method, and each row has two
switches:

| Option | Default | What it does |
|---|---|---|
| Show in library | On | Lists the set in the library's tree, its search results, its counts and its review queue |
| Include in training | On | Offers the set and its cases in the trainer's picker, including the shortcuts that select the cases you are learning or those due |

Click a puzzle or method, or focus its chevron and press `Enter` or `Space`, to
reach their sets. The switches of a puzzle or method apply to every set inside
it, so one switch can hide all of Megaminx or a whole method such as ZB. The
switches of a set only change that set, and are independent: a set can be hidden
from the library but still trained, or the other way round. Both are saved in
`cube-app.toml` under `[set_visibility.<set-id>]` once changed.

Hiding a set does not delete its statuses, algorithms, notes or review dates;
they come back when the set does.

## Keyboard shortcuts

Opens the `F1` panel, where shortcuts are rebound. See [Keyboard shortcuts](/cube-app/docs/keybinds/).

## Data

Import and export csTimer sessions, pick your own backup folder, and trigger a manual backup. See [Data](/cube-app/docs/data/).

## Updates

| Option | Default | What it does |
|---|---|---|
| Automatic checks | On | Look for new releases on GitHub |
| Channel | Stable | Also offer pre-releases |
| Frequency | Every start | Or daily, or weekly |
| Check now / skip version | — | Manual check, and clearing a skipped version |
