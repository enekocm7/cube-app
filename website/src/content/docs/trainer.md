---
title: Algorithm trainer
description: Drill the cases of any algorithm set, or whole stages of a solve, one timed attempt at a time.
section: Timing
order: 4
---

The trainer drills the cases of an algorithm set instead of full solves. Open it
with the **Trainer** tab in the title bar or `Shift + P`.

## Choosing cases

The list on the left works like the library's: each puzzle opens into its
methods, and each method into its sets. These are the sets of the
[algorithm library](/cube-app/docs/library/) for every puzzle, and the cases are
the library's own. Pick a set, then click the cases to train. Clicking a group's
name selects the whole group, and the tick above the grid selects the whole set.
What you select is remembered for next time.

Each case shows its attempts, best time, mean, and the mean of its latest five
timed attempts. The sort button above the grid lists the slowest cases first.

Sets you leave out of training in [Settings](/cube-app/docs/settings/#library)
do not appear in this list, and moving to the next set skips them. Their cases
and attempts are kept, so they come back when you include the set again.
`Ctrl + Shift + P` includes or leaves out the highlighted set.

Two shortcuts select cases from the library instead of by hand: the star button
or `3` selects the cases you are learning, and the refresh button or `4` selects
those due for review. Each replaces the selection, and tells you if the set has
no such case.

## Stages of a solve

The last set of 3x3x3 is **Stages**. Its cases have no algorithm: each is a
random state with everything before that stage solved.

- **Last layer**: the cross and the first two layers are solved.
- **F2L**: only the cross is solved.
- **Front right slot**, **Front left slot**, **Back left slot** and **Back right
  slot**: the cross and the other three slots are solved, so only that slot is
  left to do.

Select one or several and train them like any other case. Scramble with the
cross on the bottom.

## Training

Press **Start training**. The timer appears with a scramble that sets up one of
the selected cases, with a random turn of the last layer on either side. Scramble
with your last-layer color on top.

The timer works as usual, including Bluetooth timers, but inspection is off and
there are no averages. The list on the left names the case of every attempt, so
you can see which ones go well as you train.

Cases come up in random order. Every selected case comes up before any comes
back, then they all go round again in a new order. Cases that are slower than the
rest, or that you have not tried or only got a DNF on, come up twice or three
times in a round, so you practise what needs it most. The counter above the list
shows how far through the round you are. Skipping a case moves it to the end of
the round.

On the 3x3x3, the scramble sets up the case with a random turn of the free layer
on either side, and for a pair of the first two layers a random last layer. On
the 2x2x2 it is found by searching the puzzle itself. Other puzzles have no
solver here, so their scramble is the case's algorithm undone, which you may
recognise.

## The library

What you do here moves cases along in the [library](/cube-app/docs/library/).
Training a case that is not started marks it as learning. Three attempts in a row
without a DNF mark a learning case as learned, and its reviews begin. A learned
case that is due is graded by its time against the case's average: much faster
is easy, much slower is hard. A DNF on a learned case counts as forgotten, and
the case comes back sooner. Stages are not in the library and change nothing
there.

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

The book on a case opens it in the [library](/cube-app/docs/library/), where you
can add your own algorithm, which must solve the case, and choose which one to
use.

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
| Select the cases you are learning | `3` |
| Select the cases due for review | `4` |
| See the highlighted case's attempts | `Shift + Q` |
| Search the cases | `/` |
| Open the highlighted case in the library | `Shift + H` |
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
