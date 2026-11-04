# Keyboard shortcuts

cube is meant to be fully controllable from the keyboard. The timer, solves,
events, sessions, stats, settings and help all have shortcuts, dialogs stay
reachable with `Tab`, `Enter` and `Space`, every shortcut can be changed, and
the timer can be stopped with any key. The same list is available in the app
with `F1` or the keyboard icon in the title bar.

## Default bindings

Actions marked with a solve use the keyboard selection, or the latest solve when
nothing is selected.

| Action | Default | Notes |
|---|---|---|
| Start or stop the timer | `Space` | Hold and release to start the solve |
| Cancel the current attempt | `Esc` | Does not record the solve; on the stats screen it returns to the timer |
| New scramble | `N` | |
| Turn inspection on or off | `I` | |
| Select the newer solve | `Up` | Moves the highlight up the list |
| Select the older solve | `Down` | Moves the highlight down the list |
| Open the selected solve | `Enter` | Opens the details and edit dialog |
| Delete the selected solve | `D` | Asks for confirmation |
| Toggle `+2` on the selected solve | `2` | |
| Toggle DNF on the selected solve | `Shift + D` | |
| Next event | `E` | |
| Previous event | `Shift + E` | |
| Next session | `]` | |
| Previous session | `[` | |
| New session | `S` | |
| Rename session | `F2` | |
| Delete session | `Shift + S` | Asks for confirmation |
| Show stats or the timer | `A` | Toggles between the two screens |
| Open settings | `Ctrl + ,` | |
| Open the shortcuts panel | `F1` | |

While a solve is running, **any key stops the timer**. `Esc` is the exception:
it cancels the attempt without recording it.

Hold `N`, `Up` or `Down` to repeat the action until you release the key. Other
actions fire once per press.

## Changing shortcuts in the app

Press `F1` (or use the keyboard icon in the title bar) to open the shortcuts
panel. Its **Edit** button, opposite **Close**, shows a pencil on every row and
a trash can wherever the shortcut can be removed:

- The pencil starts listening for a key. Press the new combination. `Esc` or the
  **X** button cancels. Because `Esc` cancels the capture, assign `Esc` itself
  through the file instead.
- The danger-coloured trash can removes the shortcut. The action remains
  available from the mouse and can be given a new key at any time.

The key is checked before it is saved:

- It must be a key the application understands. A modifier on its own, such as
  `Shift`, is not a key.
- It must not already belong to another action. The error names the action that
  uses it. To swap two keys, remove one of them first, or set both overrides in
  the file together.
- The timer key cannot use `Ctrl`, `Alt` or `Shift`, because press and release
  must be unambiguous.
- The timer key cannot be removed; every other action can.

Valid keys include letters and numbers, `Space`, `Enter`, `Escape`, `Tab`,
`Backspace`, `Delete`, `Insert`, `Home`, `End`, `PageUp`, `PageDown`, the arrow
keys and `F1` through `F24`, each optionally with `Ctrl`, `Alt` and/or `Shift`.
On macOS the platform key (Command) is treated as `Ctrl`, and on Windows the
Windows key is too.

Changes are saved to `cube-gui.toml` immediately.

## Changing shortcuts in the file

The same settings live in a `[keybindings]` table in `cube-gui.toml`. You only
need to list the bindings you want to change; everything else keeps its default.
Hand edits are read when the app starts.

```toml
[keybindings]
next_scramble = "Ctrl + N"
next_session = "]"
delete_session = "Shift + S"
timer = "Enter"          # must not use Ctrl, Alt or Shift
```

### Syntax

- Modifiers come first, separated by `+`: `Ctrl`, `Alt`, `Shift` (or `Cmd`,
  `Super` and `Win`, which are treated as `Ctrl`). Spaces around `+` are
  optional, so `Shift + E` and `Shift+E` are the same binding.
- The key comes last: `A`, `5`, `Space`, `Enter`, `Escape`, `Up`, `F2`, and so
  on.
- Names and modifiers are case-insensitive. `Ctrl+N` and `ctrl-n` are the same
  binding. Write `Shift+N` when you need Shift; an uppercase letter alone is
  just the letter.

### Removing a binding

Set the action to an empty string or to `none`. A removed binding stays removed,
so a later version's default does not come back unexpectedly.

```toml
[keybindings]
delete_solve = ""        # no longer bound to D
next_session = "none"    # no longer bound to ]
```

The timer cannot be removed, and removing a key frees it for another action in
the same file.

### Validation

Invalid values are ignored instead of breaking the settings file. An action with
an invalid or conflicting override usually keeps its default; if another
override has claimed that default key, the action is left unassigned so two
actions never share a key. This also covers unknown action names, keys the
application does not understand, a timer key with modifiers, and two actions
asking for the same key. The shortcuts panel shows the effective bindings, so
`F1` is always the quickest way to check what is really active.

## Where the settings live

`cube-gui.toml` is in cube's user data folder, the same folder as the shared
`times.json`. On Windows that is `%APPDATA%\cube-tui`; debug builds use a
`debug` subfolder, and `CUBE_DATA_DIR` overrides the location. It is separate
from cube-tui's `config.toml`.
