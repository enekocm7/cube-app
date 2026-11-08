---
title: Relays
description: Time several puzzles as one attempt, with a split for each, from presets or your own list of events.
section: Timing
order: 6
---

A relay is several puzzles solved one after the other and timed as one attempt. cube records a split for each puzzle and
a total. Open the screen with `Ctrl + R` or **Solve → Relays** in the title bar. Everything works from the keyboard.

## Choosing a relay

Pick a relay from the dropdown at the top right of the screen, or press `1` to select the next one. The presets are:

- Runs of cubes: **2x2x2 to 4x4x4**, **2x2x2 to 5x5x5**, **2x2x2 to 6x6x6**, **2x2x2 to 7x7x7**, **3x3x3 to 5x5x5**,
  **3x3x3 to 7x7x7** and **4x4x4 to 7x7x7**.
- Short ones: 2x2x2 + 3x3x3, 3x3x3 + 3x3x3 OH, and 2x2x2 + 3x3x3 + Pyraminx + Skewb.
- Other puzzles: Megaminx + Pyraminx + Skewb, and 3x3x3 + Square-1 + Clock + Megaminx.

To make your own, choose **Create a custom relay** at the bottom of the dropdown or press `Ctrl + E`, and type its events separated by commas, in any case, such as
`3x3x3, Pyraminx, Skewb`. A range of cubes such as `2x2x2-5x5x5` stands for each cube in it. A relay has 2 to 16
puzzles, and the blindfolded events can't be part of one. Press `Ctrl + D` to delete the selected custom relay; the
attempts you made at it stay.

## Timing an attempt

1. Press the **New relay** button. cube prepares a scramble for every puzzle and shows them all on top, with the timer
   under them as on the timer screen. `Ctrl + B` copies them.
2. Press the timer key to start. With inspection on there is one inspection for the whole relay; otherwise hold and
   release as usual.
3. Press `Enter` as you finish each puzzle to keep its split and move to the next.
4. Press the timer key to stop the whole relay and record it.

If you would rather end each puzzle with the timer key, switch with the button beside the relay dropdown or `Ctrl + L`:
then every press of the timer key ends a puzzle and the last one stops the relay. The choice is saved. A puzzle you
never ended has no time of its own: the time since the last split goes to the puzzle you were on.

`Esc` abandons scrambles you have not started on, or cancels a relay that is running.

## Penalties

- An inspection `+2` counts against the first puzzle, and an inspection DNF makes the whole relay a DNF.
- After an attempt, `Left` and `Right` pick a puzzle and `2` or `Shift + D` give it a `+2` or a DNF. Press again to
  remove it.
- A `+2` adds two seconds to that puzzle's split and so to the total, so the splits always add up to the total. A DNF on
  any puzzle makes the total a DNF.

## Results

The screen lists the attempts at the selected relay, newest first, and the summary line shows the number of attempts
and the best and mean totals, leaving DNFs out of both. The selected attempt shows the split of every puzzle.

Relay attempts are kept in `relays.json`, apart from `times.json`, so they never count toward the stats of any event
and cube-tui never sees them. See [Your data](/cube-app/docs/data/).

## Editing and deleting attempts

Select an attempt with `Up` and `Down` and press `Enter` to edit it, or use the pencil button on its row. The dialog has
a field for the split of each puzzle and its penalty; the total follows the splits, so it is shown but not edited. Press
`Enter` to save or `Esc` to cancel. The trash button on the row, or `D`, deletes the attempt, asking first unless that is
turned off in settings.
