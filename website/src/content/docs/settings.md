---
title: Settings reference
description: Every option, where it lives in the dialog, and what it writes to cube-gui.toml.
section: Reference
order: 1
---

Open settings with `Ctrl + ,`. They save to `cube-gui.toml` next to your solves (see [Data](/cube-gui/docs/data/)). Shortcuts are rebound from `F1` instead — see [Keyboard shortcuts](/cube-gui/docs/keybinds/).

## Timer

| Option | Default | What it does |
|---|---|---|
| WCA inspection | On | Count down 15 s before the solve; overruns become `+2`/DNF |
| Inspection audio | Off | Beeps at 8 s and 12 s of inspection |
| Touch the digits | Off | Pressing the digits with mouse/touch works like the timer key |
| Shorter 3x3x3 scrambles | Off | Searches longer for short scrambles; noticeably slower to generate |

## Font size

| Option | Default | What it does |
|---|---|---|
| Font | All times | Cube Mono for the timer only, every time, or the whole interface, or the system fonts |
| Text size | Normal | Small / Normal / Large / Largest / Custom (8–24 pt) for the interface text |
| Timer size | Normal | Small / Normal / Large / Largest / Custom (24–240 pt) for the timer's digits |
| Scramble size | Normal | Small / Normal / Large / Largest / Custom (6–120 pt) |

## Theme

Light, dark, or system — plus your theme files. See [Themes](/cube-gui/docs/themes/).

## Appearance

| Option | Default | What it does |
|---|---|---|
| Show the scramble preview | On | Draw the scrambled puzzle under the timer and in solve details (`P` toggles it too) |
| ao5 in the solve list | Off | Adds a rolling ao5 column next to each solve |
| Always show the solve list scrollbar | Off | Keeps it visible instead of only while scrolling |
| Notifications (toasts) | On | Merge, import, delete, and update notices |
| Confirm before deleting | On | Asks before deleting a solve or session; Undo works either way |

## Updates

| Option | Default | What it does |
|---|---|---|
| Automatic checks | On | Look for new releases on GitHub |
| Channel | Stable | Also offer pre-releases |
| Frequency | Every start | Or daily, or weekly |
| Check now / skip version | — | Manual check, and clearing a skipped version |

## Language

Sixteen languages, including English, Spanish, German, French, Japanese, and Chinese. Dates and counts follow the chosen language. Empty (the default) follows the system.

## Data

Import and export csTimer sessions, pick your own backup folder, and trigger a manual backup. See [Data](/cube-gui/docs/data/).
