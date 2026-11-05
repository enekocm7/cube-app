---
title: Keyboard shortcuts
description: Every action has a shortcut, every shortcut can be rebound, and the whole app works without a mouse.
section: Timing
order: 2
---

cube stays fully usable from the keyboard. The same list lives in the app under `F1` (or the keyboard icon in the title bar). The panel opens with a search field — press `/` to focus it and filter by action or key.

## Timer

| Action | Default |
|---|---|
| Start inspection, hold and release to start, press to stop | `Space` |
| Stop a running solve | Any key |
| Cancel the attempt without recording it | `Esc` |
| New scramble (hold to keep generating) | `N` |
| Turn inspection on or off | `I` |
| Show or hide the scramble preview | `P` |
| Manual mode on or off | `M` |

## Solves and sessions

| Action | Default |
|---|---|
| Select newer / older solve | `Up` / `Down` |
| Open the selected solve | `Enter` |
| Delete the selected solve | `D` |
| Toggle `+2` | `2` |
| Toggle DNF | `Shift + D` |
| Next / previous event | `E` / `Shift + E` |
| Next / previous session | `]` / `[` |
| New session | `S` |
| Rename session | `F2` |
| Delete session | `Shift + S` |
| Undo deletion (repeatable) | `Ctrl + Z` |
| Redo an undone deletion | `Ctrl + Y` |

## Stats and charts

| Action | Default |
|---|---|
| Shorter / longer average | `Left` / `Right` |
| Open selected average, or its best | `O` / `Shift + O` |
| Copy average as text | `C` |
| Stats or timer | `A` |
| Charts or timer | `G` |
| Next charts view | `V` |
| Filter charts by event | `F` |
| Filter by range / custom dates | `R` / `Shift + R` |
| One session or all | `W` |
| Zoom times chart in / out / reset | `Z` / `Shift + Z` / `X` |

## App

| Action | Default |
|---|---|
| Settings | `Ctrl + ,` |
| Shortcuts panel | `F1` |
| Bluetooth timer | `B` |

## Rebinding in the app

In the `F1` panel, **Edit** (opposite **Close**) shows a pencil per row and a trash can wherever the shortcut can be removed:

- The pencil listens for your next key. `Esc` cancels (assign `Esc` itself through the file instead).
- The trash can unbinds the action. It stays usable from the mouse and can be rebound any time.
- Validation runs before saving: the key must be understood, must not belong to another action, the timer key must not use modifiers, and the timer cannot be unbound.

## Rebinding in the file

The same settings live in a `[keybindings]` table in `cube-gui.toml`. List only what you change:

```toml
[keybindings]
next_scramble = "Ctrl + N"
timer = "Enter"          # must not use Ctrl, Alt or Shift
delete_solve = ""        # unbound; "none" works too
```

Modifiers come first (`Ctrl + Shift + E`), the key comes last. Names are case-insensitive, and `Cmd`/`Super`/`Win` count as `Ctrl`. A removed binding stays removed so a later default never returns unexpectedly.
