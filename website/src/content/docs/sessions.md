---
title: Solves and sessions
description: Edit any solve, penalize it, move between sessions, and undo deletions.
section: History
order: 1
---

## Sessions

- Sessions group your solves — one per event, one for practice, one for competition prep, whatever you like.
- Switch with `]` / `[`, or the session dropdown. `S` creates one, `F2` renames it, `Shift + S` deletes it.
- Deleting asks for confirmation (toggleable) and shows an Undo toast. `Ctrl + Z` / `Ctrl + Y` step through deletions.

## Solve details

`Enter` (or clicking a solve) opens its details. Everything is editable:

- **Time** — fix a mistyped manual entry.
- **Event** — move the solve to the right puzzle.
- **Scramble** — the preview follows what you type.
- **Penalty** — None, `+2`, or DNF.
- **Date** — backdate a solve imported from elsewhere.
- **Comment** — context like "lucky F2L" or "new cube".

Every change is kept in an audit trail, so you can see what a solve used to be.

## Penalties

- `2` toggles `+2` on the keyboard selection (or the latest solve).
- `Shift + D` toggles DNF.
- Inspection overruns apply them automatically: +2 past 15 s, DNF past 17 s.

## Selecting solves

- `Up` / `Down` moves the highlight in the solve list (hold to keep moving).
- Actions that need a solve use the highlight, or the latest solve when nothing is highlighted.
- In an average's dialog, each solve is a button: reach it with `Tab`, open it with `Enter` or `Space`.
