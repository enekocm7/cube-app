---
title: Algorithm library
description: Browse the algorithm sets of every puzzle, choose your algorithms and review what you have learned.
section: Timing
order: 6
---

The library lists the algorithm sets of every puzzle. Open it with the
**Library** tab in the title bar or `Shift + B`.

## Training a case

The **Train** button, or `5`, on the highlighted case opens the [trainer](/cube-app/docs/trainer/)
with only that case selected and starts drilling it.

## Browsing

The list on the left goes from puzzle to method to set, as dropdowns that show
how many cases you have learned. Pick a set to see its cases, or press
`Shift + U` for the next puzzle and `Y` for the next set, which open the way to
it and close the other puzzles.
`Up` and `Down` move through the cases.

The search field looks through the cases of every set, by case, group, set,
method and puzzle. Press `/` to reach it and `Esc` to clear it.

## Hiding a set

Some sets are huge or already learned. In [Settings](/cube-app/docs/settings/#library),
each set has two switches:

- **Show in library** keeps the set out of the tree, the search, the counts and
  the review queue when off.
- **Include in training** keeps the set out of the trainer's picker when off,
  including the shortcuts that select cases from the library.

Both are on by default and work separately. From the library, `Ctrl + Shift + B`
switches the highlighted set's library visibility and `Ctrl + Shift + P` switches
its place in the trainer; the same keys work in the trainer. The set's data is
kept while it is hidden.

## A case

The panel on the right shows the highlighted case: its drawing, what to look
for, every algorithm it has and your note. Its status is the dropdown at the end
of its row in the list.

- `Left` and `Right` highlight an algorithm.
- `Shift + F` makes the highlighted algorithm your favourite, which is the one
  shown in the list. Press it again to go back to the first one.
- `Shift + I` adds your own algorithm. It is checked against the case first, on
  every puzzle.
- `D` removes the highlighted algorithm, if it is one of yours.
- `.` edits your note.
- `Shift + X` moves the case from not started to learning to learned.

The trainer and the library share the PLL and OLL cases, and the algorithm you
choose in one is the one the other uses.

## Review

A learned case comes back for review on a schedule that stretches as you
remember it. The button above the cases says how many are due. Press
`Shift + W` to start.

Each case shows its drawing. Try it, then press `H`, `Enter` or the timer key to
see the algorithm, and grade how it went: `6` again, `7` hard, `8` good or
`9` easy. `Esc` leaves the review.

## Where the algorithms come from

Most sets come from [SpeedCubeDB](https://speedcubedb.com/), with its methods,
sets, case names and groups. The 2-look sets and the 6x6x6, 7x7x7 and Clock sets
were written by hand. The Skewb, FTO, Roux LSE, Pyraminx first layer and
Square-1 CSP sets were found by a search, so their cases are numbered instead of
having community names, and their algorithms are short but not always
comfortable. Add your own where you know a better one.

Every algorithm listed is run on a model of its puzzle by the tests.

Your statuses, algorithms, notes and review dates are saved in `library.json`.
