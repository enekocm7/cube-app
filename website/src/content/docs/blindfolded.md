---
title: Blindfolded events
description: 3x3x3, 4x4x4 and 5x5x5 Blindfolded with a memo split, the stats that suit them, and Multi-Blind scored under WCA rules.
section: Timing
order: 5
---

cube has three blindfolded events, **3x3x3 Blindfolded**, **4x4x4 Blindfolded** and **5x5x5 Blindfolded**, and a
[Multi-Blind](#multi-blind) mode. Pick an event with `E` and `Shift + E` like any other.

## Timing an attempt

Blindfolded events have no inspection: you look at the cube while the clock runs, which is how the WCA times them.

1. Hold and release the timer key to start. The memo is running.
2. Press the timer key once when you have memorized the cube and put the blindfold on. The digits keep running and the
   memo time is marked.
3. Press any key when the cube is solved to stop.

If you stop without marking the memo, the solve is recorded with its total time only. `Esc` cancels the attempt. `+2` and
DNF work on a solve as usual; a DNF is how you record a failed attempt.

A hardware timer (Bluetooth), manual mode and a time typed in by hand have no memo, so they are recorded with a total only.
The memo of a solve is kept beside your tags (see [Your data](/cube-app/docs/data/)), and its details show the memo and the
execution.

## Statistics

An average of 5 says little when many attempts are DNF, so these events show something else in the stats panel:

- **Success rate**: the share of attempts that were not DNF, as a whole percentage.
- **Best single**: the fastest success.
- **Mean of 3**: current and best. A DNF in any of the three makes it a DNF.
- **Memo** and **execution**: the mean of the successes timed in two parts, and the shortest memo.

The stats table and the charts treat these events like any other, so the times chart and the calendar include them.
Goals and personal bests work the same way too.

## Multi-Blind

Press `Shift + M` or use the Multi-Blind tab, beside Trainer, to open the screen. It has its own attempts, kept apart from
your solves, because the result is points and not a time.

1. Press the timer key (or **New Multi-Blind attempt**) and choose how many cubes (2 to 100).
2. cube prepares one scramble per cube. Copy them with `Shift + C`, or save them to a text file with `Shift + V` to print.
3. Press the timer key to start, the timer key again when you finish.
4. Enter how many cubes you solved. `Enter` saves the attempt.

`Esc` backs out of scrambles you have not started on. A stopped attempt stays until you enter its result.

### Scoring

cube scores attempts with the WCA rules:

- Points are the cubes solved minus the cubes unsolved.
- An attempt with zero or fewer points is a DNF.
- The time limit is 10 minutes per cube attempted, up to 60 minutes. An attempt over the limit is a DNF. The limit for the
  attempt is shown while you time it.
- A better result means more points, then less time.

The screen summarizes your attempts, how many were scored and the best result, and lists them newest first. `Up` and `Down`
select an attempt, `Enter` edits it (cubes attempted, cubes solved and time, which are scored again) and `D` deletes it. Each row has a pencil and a trash can for the mouse.

## Compatibility

The blindfolded events are the ones cube-tui has, so their solves appear there with their total time. Multi-Blind attempts
stay in cube. A csTimer import infers the event from the scramble, and csTimer does not tell blindfolded scrambles from
regular ones, so imported blindfolded solves arrive as the regular event. Edit the event of a solve in its details to
fix one.
