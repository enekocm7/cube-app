---
title: Algorithm trainer
description: Drill PLL and OLL cases, one timed attempt at a time, and see which are slowest.
section: Timing
order: 4
---

The trainer drills the cases of an algorithm set instead of full solves. Open it
with the **Trainer** tab in the title bar or `Shift + P`.

## Choosing cases

The list on the left shows each puzzle and its sets: PLL with its 21 cases and
OLL with its 57. Pick a set, then click the cases to train. Clicking a group's
name selects the whole group, and the tick above the grid selects the whole set.
What you select is remembered for next time.

Each case shows its attempts, best time, mean, and the mean of its latest five
timed attempts. The sort button above the grid lists the slowest cases first.

## Training

Press **Start training**. The timer appears with a scramble that sets up one of
the selected cases, with a random turn of the last layer on either side. Scramble
with your last-layer color on top.

The timer works as usual, including Bluetooth timers, but inspection is off and
there are no averages. The list on the left names the case of every attempt, so
you can see which ones go well as you train.

Cases come up in random order. Every selected case comes up once before any comes
back, then they all go round again in a new order. The counter above the list
shows how far through the round you are. Skipping a case moves it to the end of
the round.

The eye button shows the case if you are stuck. Click an attempt in the list, or
select it and press `Enter`, to see its case, algorithm and scramble.

`+2`, DNF and delete work on the selected attempt, or the latest one. Deleting an
attempt cannot be undone.

## Finding cases

The search field above the cases finds them by name or by group, such as `Aa` or
`edges only`. Press `/` to reach it, then `Enter` or `Down` to move on to what it
found. `Esc` ends the search. Selecting every case or a group only touches the
cases the search left on screen.

## Past attempts

The timer button on a case, or `Shift + Q` on the highlighted one, lists every
attempt at that case from all your drills, newest first, with its date. Select
one to change its penalty or delete it. Deleting an attempt also takes it out of
the case's figures. `Esc` goes back to the cases.

## Your own algorithms

The pencil on a case replaces its algorithm with your own. It must solve the case,
and cube tells you if it does not or if a move can't be read. Wide moves, slices
and rotations all work. Leave the field empty to go back to the default.

## Keyboard

| Action | Default |
|---|---|
| Show the trainer or the timer | `Shift + P` |
| Move through the cases | Arrow keys |
| Select or clear the highlighted case | `J` or `Space` |
| Select or clear its group | `Shift + J` |
| Select or clear every case | `Shift + A` |
| Next set | `Y` |
| Sort by slowest | `Shift + Y` |
| See the highlighted case's attempts | `Shift + Q` |
| Search the cases | `/` |
| Edit the highlighted case's algorithm | `Shift + H` |
| Start training | `Q` or `Enter` |
| Show or hide the case | `H` |
| Skip the case | `N` |
| Select an attempt | `Up` and `Down` |
| See an attempt's case | `Enter` |
| Back to the cases | `Esc` |

## Your data

Attempts, selections and your algorithms are kept in `trainer.json`, apart from
`times.json`. They never appear in sessions, averages or charts, and cube-tui
does not read them.
