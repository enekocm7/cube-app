---
title: FAQ and troubleshooting
description: Fixes for SmartScreen warnings, unreadable saves and shortcuts that stop working.
section: Start here
order: 2
---

## Windows warns about an unknown publisher

The installer and portable builds are **unsigned**, so SmartScreen warns on first run. This is expected until releases are code-signed:

1. Click *More info*.
2. Click *Run anyway* if you downloaded from [GitHub Releases](https://github.com/enekocm7/cube-gui/releases).

## "Your history couldn't be read"

If `times.json` exists but can't be parsed, cube shows an error and turns saving **off**. Your file is never overwritten with an empty history.

To recover:

1. Close cube and cube-tui.
2. Copy a file from `backups/` (or your own backup folder) over `times.json`.
3. Reopen cube.

## I ran cube and cube-tui at the same time

Both apps rewrite the whole file on save, so whoever saves last wins at file granularity — but cube merges what cube-tui wrote and notifies you. Rules of thumb:

- Let one app finish saving before editing in the other.
- Delete solves in only one app at a time, with the other closed.
- When in doubt, back up `times.json` first.

## Does it run on Linux and macOS?

Packages are built for Linux x64 and macOS (Apple silicon), but they have not been fully tested. Expect rough edges around system fonts, packaging (no `.app`/`.dmg` or `.desktop` file yet), and Bluetooth permissions. Please include your OS version in bug reports.

## The timer key feels wrong

- Hold time is tuned like a stackmat: red means keep holding, green means release.
- If inspection is on, the first press starts inspection, not the solve.
- In manual mode (`M`) the timer key is disabled by design — press `M` again to exit.
- While a Bluetooth timer is connected, the keyboard never starts or stops solves.

## Shortcuts stopped working in a dialog

Dialogs reserve `Enter`, `Esc`, `Tab` and `Space` for the dialog itself. `Enter` runs the primary action, `Esc` cancels, and `Enter`/`Space` on a focused control activates that control. The app shortcuts resume once the dialog closes.
