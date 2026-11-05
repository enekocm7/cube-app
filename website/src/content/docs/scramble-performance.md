---
title: Scramble performance
description: How fast official scrambles generate, and why the app precomputes them per event.
section: Reference
order: 5
---

Numbers for comparing future changes. They come from an AMD Ryzen 7 5800X3D (16 threads) on Windows 11, with `tnoodle-rs` 0.1.1 and its default `parallel` feature, which must stay on. Timings on this machine vary by 20% or more between runs.

Regenerate the tables with:

```sh
cargo run --release -p cube-core --example scramble_bench
```

## Solver

Mean of 20 scrambles after the first call, in the release profile. The first call includes building the lookup tables. *Shorter 3x3x3 scrambles* takes at least 200 ms by design.

| event | first call (incl. tables) | steady state |
|---|---|---|
| Cube2x2 | 8.02 ms | 8.09 ms |
| Cube3x3 | 26.74 ms | 0.58 ms |
| Cube4x4 | 117.37 ms | 42.43 ms |
| Cube5x5 | 1.21 ms | 1.33 ms |
| Cube6x6 | 4.05 ms | 3.09 ms |
| Cube7x7 | 4.43 ms | 4.46 ms |
| Megaminx | 1.95 ms | 1.89 ms |
| Pyraminx | 0.20 ms | 0.08 ms |
| Fto | 360.52 ms | 85.38 ms |
| Skewb | 0.55 ms | 0.08 ms |
| Square1 | 14.50 ms | 7.72 ms |
| Clock | 0.48 ms | 0.39 ms |
| Cube3x3 (shorter scrambles) | 200.17 ms | 200.21 ms |

## Queue

How long getting a scramble takes from `ScrambleQueue`. *Scramble prepared* is the usual case.

| event | scramble prepared | queue empty |
|---|---|---|
| Cube2x2 | 0.01 ms | 10.76 ms |
| Cube3x3 | 0.00 ms | 1.07 ms |
| Cube4x4 | 0.01 ms | 11.27 ms |
| Cube5x5 | 0.01 ms | 1.37 ms |
| Cube6x6 | 0.01 ms | 2.69 ms |
| Cube7x7 | 0.01 ms | 5.44 ms |
| Megaminx | 0.01 ms | 2.25 ms |
| Pyraminx | 0.01 ms | 0.22 ms |
| Fto | 0.01 ms | 10.57 ms |
| Skewb | 0.01 ms | 0.43 ms |
| Square1 | 0.00 ms | 17.85 ms |
| Clock | 0.01 ms | 0.43 ms |

## Queue design

- **Push, not poll.** `next_scramble` takes a prepared scramble synchronously, and `ScrambleSignal` wakes the UI as soon as a worker delivers one.
- **Two ahead.** `SCRAMBLES_AHEAD` covers skipping and then starting an attempt.
- **Kept per event.** Prepared scrambles survive event switches, so returning to an event is instant and shows the same scramble.
- **One worker per event, started on first use.** A single shared worker was tried and rejected: a cold FTO solve delayed the next events by up to a second.
- **No prefetching of other events.** Building every table at startup would make first visits instant but costs CPU and memory for events never used.
