---
title: Keyboard shortcuts
description: Every action has a shortcut, every shortcut can be rebound, and the whole app works without a mouse.
section: Timing
order: 2
---

cube is meant to be fully controllable from the keyboard. The timer, solves,
events, sessions, stats, settings and help all have shortcuts, dialogs stay
reachable with `Tab`, `Enter` and `Space`, every shortcut can be changed, and
the timer can be stopped with any key. The same list is available in the app
with `F1` or the keyboard icon in the title bar.
The panel opens with a search field; press `/` to focus it (the
"Search in dialogs" shortcut, which you can change or remove). Type to filter by action name or current
key (for example `space`, `ctrl` or `unassigned`). While an action is being
rebound, keys go to the capture, never to the search field.

## Default bindings

Actions marked with a solve use the keyboard selection, or the latest solve when
nothing is selected.

| Action | Default | Notes |
|---|---|---|
| Start or stop the timer | `Space` | Hold and release to start the solve. Pressing and releasing the digits with the left mouse button or a touch does the same while **Timer → Touch the digits to start or stop the timer** is on (off by default) |
| Cancel the current attempt | `Esc` | Does not record the solve; on the stats, charts, goals, library or Multi-Blind screen it returns to the timer. In the trainer it goes from the drill back to the cases, then to the timer. In Multi-Blind it abandons scrambles you have not started on |
| New scramble | `N` | |
| Turn inspection on or off | `I` | |
| Show or hide the scramble preview | `P` | The same switch as the setting |
| Turn manual mode on or off | `M` | Type a time from another timer. See [Manual mode](#manual-mode) |
| Select the newer solve | `Up` | Moves the highlight up the list, or up the stats table. See [Charts](#charts) for the charts screen |
| Select the older solve | `Down` | Moves the highlight down the list, or down the stats table |
| Open the selected solve | `Enter` | Opens the details and edit dialog, from the solve list or the stats table |
| Delete the selected solve | `D` | Asks for confirmation (can be turned off in settings) and shows an Undo toast. Each entry in the session dropdown also has a trash button, reachable with `Tab` and pressed with `Enter` or `Space` |
| Toggle `+2` on the selected solve | `2` | |
| Toggle DNF on the selected solve | `Shift + D` | |
| Tag the selected solve | `T` | Works on the timer and stats screens. See [Tags](#tags) |
| Choose tags for the next solves | `Shift + T` | They stay on until changed. See [Tags](#tags) |
| Rename or delete tags | `Ctrl + T` | See [Tags](#tags) |
| Next event | `E` | |
| Previous event | `Shift + E` | |
| Next session | `]` | |
| Previous session | `[` | |
| New session | `S` | |
| Rename session | `F2` | |
| Delete session | `Shift + S` | Asks for confirmation (can be turned off in settings) and shows an Undo toast |
| Undo the last solve or session deletion | `Ctrl + Z` | Repeat to step back through earlier deletions |
| Redo an undone deletion | `Ctrl + Y` | A new deletion clears the redo history |
| Select the shorter average | `Left` | Moves between mo3, ao5, ao12, ao50 and ao100, highlighted in the stats panel and the stats table |
| Select the longer average | `Right` | |
| Open the selected average | `O` | Lists the solves of the average ending at the selected solve, marking the trimmed ones. Starts on ao5 |
| Open the best of the selected average | `Shift + O` | |
| Create an image of the selected solve | `K` | Draws the solve's card and opens it in the image preview: `Enter` copies it, `Tab` reaches **Download PNG**, **Close** and copying its link, and `Esc` closes. **Create image** in the solve's dialog does the same |
| Copy a link to the selected solve | `U` | Copies an [alg.cubing.net](https://alg.cubing.net) link with the scramble, the solution if the solve has one, and the comment. Only for 2x2x2 to 7x7x7, the puzzles the site has |
| Copy the selected average as text | `C` | Copies the result and every attempt with its scramble. The average's dialog has the same action on a button |
| Create an image of the selected average | `Shift + K` | Draws the card of mo3, ao5 or ao12 and opens it in the image preview, with the same controls and a link to its fastest attempt. Longer averages have too many attempts for a card. **Create image** in the average's dialog does the same |
| Show stats or the timer | `A` | Toggles the table of every solve's averages |
| Show charts or the timer | `G` | |
| Connect a Bluetooth timer | `B` | Also the Bluetooth icon in the title bar. See [Bluetooth timers](#bluetooth-timers) |
| Show goals or the timer | `Shift + G` | Also the tick icon in the title bar. See [Goals](#goals) |
| New goal | `Shift + N` | Works on the goals screen. See [Goals](#goals) |
| Show the trainer or the timer | `Shift + P` | See [Trainer](#trainer) |
| Next algorithm set | `Y` | Works on the trainer's cases. See [Trainer](#trainer) |
| Select or clear the highlighted case | `J` | Works on the trainer's cases, where the timer key does the same |
| Select or clear the highlighted case's group | `Shift + J` | Works on the trainer's cases |
| Select or clear every case | `Shift + A` | Works on the trainer's cases |
| Start training | `Q` | Works on the trainer's cases, where `Enter` does the same |
| Show or hide the case | `H` | Works during a drill |
| Edit the algorithm | `Shift + H` | Works on the trainer's cases, for the highlighted one |
| Sort by slowest | `Shift + Y` | Works on the trainer's cases |
| See the case's attempts | `Shift + Q` | Works on the trainer's cases |
| Show the library or the timer | `Shift + B` | Also the Library tab in the title bar. See [Library](#library) |
| Next puzzle | `Shift + U` | Works in the library |
| Change the case's status | `Shift + X` | Works in the library, for the highlighted case |
| Favourite algorithm | `Shift + F` | Works in the library, for the highlighted algorithm |
| Add algorithm | `Shift + I` | Works in the library, for the highlighted case |
| Edit note | `.` | Works in the library, for the highlighted case |
| Start the review | `Shift + W` | Works in the library |
| Again | `6` | Grades a case in a review, once its algorithm is showing |
| Hard | `7` | Grades a case in a review, once its algorithm is showing |
| Good | `8` | Grades a case in a review, once its algorithm is showing |
| Easy | `9` | Grades a case in a review, once its algorithm is showing |
| Show Multi-Blind or the timer | `Shift + M` | Also the Multi-Blind tab in the title bar. See [Multi-Blind](#multi-blind) |
| Copy the Multi-Blind scrambles | `Shift + C` | Works on the Multi-Blind screen, once an attempt has its scrambles |
| Save the Multi-Blind scrambles | `Shift + V` | Writes them to a text file, one per cube, to print |
| Show the next charts view | `V` | Cycles Trends, Calendar, Compare, Tags and All-time bests |
| Filter charts by the next event | `F` | Cycles all events and every event with solves |
| Filter charts by the next range | `R` | Cycles all solves, the last 50, 100 or 500 solves and the last 7, 30, 90 or 365 days |
| Filter charts by custom dates | `Shift + R` | Asks for a first and last day as `YYYY-MM-DD` |
| Filter by the next tag | `L` | Cycles all tags and every tag. Works on the stats and charts screens, except All-time bests |
| Compare with the next tag | `Shift + L` | Works on the Tags chart. See [Tags](#tags) |
| Chart one session or all of them | `W` | With all of them, solves merge by date and averages stay inside each session. `[` and `]` go back to one session |
| Zoom in on the times chart | `Z` | Hold to keep zooming. See [Charts](#charts) |
| Zoom out of the times chart | `Shift + Z` | Hold to keep zooming |
| Reset the times chart zoom | `X` | |
| Open settings | `Ctrl + ,` | |
| Open the shortcuts panel | `F1` | |
| Search in dialogs | `/` | Jumps to the search field of the shortcuts panel, the settings dialog, the trainer's cases and the library. It types normally once the field has focus |

While a solve is running, **any key stops the timer**. `Esc` is the exception:
it cancels the attempt without recording it.

Hold `N`, `Up` or `Down` to repeat the action until you release the key. Other
actions fire once per press.

The solves listed in an average's dialog are buttons: reach them with `Tab` and
press `Enter` or `Space` to open that solve.

## Goals

On the goals screen, `Up` and `Down` select a goal, `Enter` edits it, `D` deletes
it and `Shift + N` adds a new one. They follow the bindings of selecting, opening
and deleting a solve, so rebinding those moves these too.

In the goal dialog, `Tab` moves between the controls, `Enter` or `Space` presses
the focused one and `Enter` anywhere else saves.

## Trainer

On the trainer's cases, the arrow keys move the highlight: the bindings of
selecting solves and averages move it up and down, and left and right. During a
drill, new scramble skips the case, and selecting, opening, deleting and the
penalties work on its attempts. Selecting, deleting and the penalties work the
same on a case's attempts. `Esc` goes back to the cases. See
[Algorithm trainer](/cube-app/docs/trainer/).

## Library

In the library, the bindings of selecting solves move through the cases and
those of selecting averages through the highlighted case's algorithms. `Y` is
the next set and deleting a solve removes the highlighted algorithm if it is
yours. In a review, showing the case, `Enter` and the timer key show the
algorithm. `Esc` ends a review, then clears the search, then returns to the
timer. See [Algorithm library](/cube-app/docs/library/).

## Multi-Blind

On the Multi-Blind screen, `Up` and `Down` select an attempt, `Enter` opens it to edit (the newest when none is selected) and `D` deletes it. They follow the bindings of selecting, opening and deleting a solve. The timer key starts a new attempt, then starts and stops the timer once the scrambles are ready; when the timer has stopped, `Enter` enters how many cubes you solved. In the dialogs, `Tab` moves between the fields and `Enter` saves. See [Blindfolded events](/cube-app/docs/blindfolded/).

## Tags

`T` and `Shift + T` open the same dialog, on the selected solve or on the next
solves. The field starts focused: type a tag and press `Enter` to add it, then
`Enter` again to save. `Shift + Tab` and `Tab` reach the tags already known, and
`Enter` or `Space` turns the focused one on or off. Solve details have the same
controls, which save along with the rest of the solve.

`Ctrl + T` lists every tag with the solves using it. Each row has a pencil and a
trash can, reachable with `Tab` and pressed with `Enter` or `Space`. Deleting a
tag that solves use asks first.

`L` filters the stats table and the charts together. On the Tags chart it picks
the left side of the comparison and `Shift + L` the right one. Either side can be
all tags, which compares a tag with every solve.

## Charts

`V`, `F`, `R`, `Shift + R`, `W`, `Z`, `Shift + Z` and `X` work on the charts screen
and do nothing elsewhere.
The arrow keys depend on the view:

- On Calendar, `Left` and `Right` move the selected day by one and `Up` and
  `Down` by a week. The line below the calendar shows that day's solves.
- On every other view, `Up` and `Down` scroll.
- On Trends, `Left` and `Right` pan the times chart while it is zoomed.

With the mouse, turn the wheel over the times chart to zoom around the cursor and
hold `Shift` to pan. Zoomed all the way out, turning the wheel down scrolls the
page instead. Changing a filter resets the zoom.

## Manual mode

Press `M` on the timer screen to type in times taken on another timer, as in
cube-tui. The timer key is disabled and the digits show what you type:

- Digits, `:` and `.` build the time (`8.98`, `1:32.49`, `12:34`), up to two
  digits for each of minutes and seconds and three for the decimals. `Backspace`
  deletes and `Ctrl + V` pastes.
- `Enter` records the solve against the current scramble and moves on to the
  next one, staying in manual mode. An invalid time such as `12:99` is kept and
  the error is shown until you edit it.
- With nothing typed, `Enter` still opens the selected solve's details.
- Every other shortcut keeps working. Use `Up`/`Down` to select the entered
  solve, then `Shift + D` for DNF or the details dialog for `+2` (the digit
  shortcut `2` types a digit while manual mode is on).
- Press `M` again to return to the timer.

## Bluetooth timers

Press `B` to look for GAN timers. Turn yours on and it appears in the list:

- `Up` and `Down` select a timer and `Enter` connects to it. Each timer is also
  a button, reachable with `Tab` and pressed with `Enter` or `Space`.
- The dialog closes once the timer is connected. Closing it while connecting
  leaves the connection going, and the title bar icon shows how it went.
- `B` again shows the connected timer, and `Enter` disconnects it. While
  connecting, `Enter` cancels.

While a timer is connected or connecting, the timer key and the digits do
nothing: the solve starts and stops on the timer, and the digits follow it.
`Esc` still cancels an attempt, so it is not recorded when the timer finishes.
Inspection is not used. Every other shortcut works as usual.

## Keys inside dialogs

These keys are fixed, and the shortcuts above stand down while a dialog is open.

| Key | What it does |
|---|---|
| `Enter` | Runs the dialog's primary action: Save, Delete or Update now. A dialog with no primary action closes |
| `Esc` | Cancels and closes the dialog, and never runs the primary action |
| `Tab` / `Shift + Tab` | Moves between the dialog's controls |
| `Enter` or `Space` on a focused control | Activates that control instead, so `Enter` on Cancel cancels |

A form that fails validation stays open and shows the error. In a multi-line
field `Enter` inserts a newline, and in an open menu it picks the highlighted
item. While the shortcuts panel waits for a key, `Enter` and `Esc` go to the
capture rather than the dialog.

The settings dialog opens on its category list, which is one `Tab` stop:

| Key | What it does |
|---|---|
| `Up` / `Down` | Shows the previous or next category, wrapping at the ends |
| `Home` / `End` | Shows the first or last category |
| `Tab`, `Right`, `Enter` or `Space` | Moves from the list to the first setting |
| `Left` | Moves from a setting back to the list, except in a text field |
| `/` | Jumps to the search field, unless you rebind "Search in dialogs" |

While you search, the list keeps only the categories with a match and the right
pane shows matches from all of them. Picking a category ends the search.

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

Changes are saved to `cube-app.toml` immediately.

## Changing shortcuts in the file

The same settings live in a `[keybindings]` table in `cube-app.toml`. You only
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

`cube-app.toml` is in cube's user data folder, the same folder as the shared
`times.json`. On Windows that is `%APPDATA%\cube-tui`; debug builds use a
`debug` subfolder, and `CUBE_DATA_DIR` overrides the location. It is separate
from cube-tui's `config.toml`.
