---
title: Scramble performance
description: How fast official scrambles generate, and why the app precomputes them per event.
section: Reference
order: 5
---

Numbers for comparing future changes, recorded for issue #70. They come from
an AMD Ryzen 7 5800X3D (16 threads) on Windows 11, with `tnoodle-rs` 0.1.1 and
its default `parallel` feature, which must stay on. Timings on this machine
vary by 20% or more between runs, and 4x4, FTO and Square-1 vary the most, so
compare medians of several interleaved runs rather than two single runs.

Regenerate the first two tables with:

```sh
cargo run --release -p cube-core --example scramble_bench
```

## Solver

Mean of 20 scrambles after the first call, in the release profile. The first
call includes building the lookup tables. *Shorter 3x3x3 scrambles* takes at
least 200 ms by design.

| event | first call (incl. tables) | steady state |
|---|---|---|
| Cube2x2 | 8.60 ms | 7.86 ms |
| Cube3x3 | 26.77 ms | 0.68 ms |
| Cube3x3OneHanded | 0.12 ms | 0.41 ms |
| Cube3x3Blind | 2.94 ms | 0.72 ms |
| Cube4x4 | 114.15 ms | 42.47 ms |
| Cube4x4Blind | 28.22 ms | 31.47 ms |
| Cube5x5 | 1.53 ms | 1.47 ms |
| Cube5x5Blind | 1.87 ms | 1.87 ms |
| Cube6x6 | 2.65 ms | 2.64 ms |
| Cube7x7 | 4.05 ms | 4.56 ms |
| Megaminx | 1.94 ms | 2.16 ms |
| Pyraminx | 0.26 ms | 0.07 ms |
| Fto | 532.81 ms | 53.73 ms |
| Skewb | 0.45 ms | 0.07 ms |
| Square1 | 11.73 ms | 7.38 ms |
| Clock | 0.29 ms | 0.23 ms |
| Cube3x3 (shorter scrambles) | 200.18 ms | 200.17 ms |

## Queue

How long getting a scramble takes from `ScrambleQueue`. *Scramble prepared* is
the usual case. *Queue empty* waits on `ScrambleSignal` after draining the queue,
so it is one sample that can start partway through a solve already running.

| event | scramble prepared | queue empty |
|---|---|---|
| Cube2x2 | 0.01 ms | 10.16 ms |
| Cube3x3 | 0.01 ms | 0.42 ms |
| Cube3x3OneHanded | 0.00 ms | 0.69 ms |
| Cube3x3Blind | 0.01 ms | 0.76 ms |
| Cube4x4 | 0.01 ms | 16.48 ms |
| Cube4x4Blind | 0.01 ms | 39.06 ms |
| Cube5x5 | 0.01 ms | 1.68 ms |
| Cube5x5Blind | 0.01 ms | 1.43 ms |
| Cube6x6 | 0.01 ms | 2.61 ms |
| Cube7x7 | 0.01 ms | 4.46 ms |
| Megaminx | 0.01 ms | 2.22 ms |
| Pyraminx | 0.01 ms | 0.15 ms |
| Fto | 0.01 ms | 23.27 ms |
| Skewb | 0.01 ms | 0.17 ms |
| Square1 | 0.01 ms | 2.96 ms |
| Clock | 0.01 ms | 0.34 ms |

## In the app

Time from the request to the frame that shows the scramble, measured with
temporary probes in a dev build while posting `N` and `E` key presses to the
window. Dev builds compile `tnoodle-rs` at `opt-level = 3`.

| case | samples | result |
|---|---|---|
| Scramble prepared (skip, or event visited before) | 29 | median 6.1 ms, worst 26 ms |
| First visit to an event, tables included | 12 | 2 to 45 ms, 4x4 127 ms, FTO 567 ms |

A prepared scramble now reaches the next frame. It used to wait up to 100 ms
for the idle tick as well.

cube-tui takes its ready scramble synchronously and solves on the calling
thread if none is ready, using the same solver and the same one-solve cost. The
two apps were not timed side by side, because cube-tui has no equivalent probe.

## Queue design

- **Push, not poll.** `next_scramble` takes a prepared scramble synchronously,
  and `ScrambleSignal` wakes the UI as soon as a worker delivers one.
- **Two ahead.** `SCRAMBLES_AHEAD` covers skipping and then starting an attempt.
- **Kept per event.** Prepared scrambles survive event switches, including the
  one on screen, so returning to an event is instant and shows the same scramble.
- **One worker per event, started on first use.** A worker whose event is not
  current finishes its running solve and then sleeps, so cycling events runs at
  most one solve per event. A single shared worker was tried first and rejected:
  a cold FTO solve delayed the next events by up to about a second.
- **No prefetching of other events.** Building every table at startup would make
  first visits instant, but costs CPU and memory for events that may never be used.

## Release profile

Steady state, median of 6 interleaved runs of the benchmark, in ms.

| event | `lto = "thin"`, dependencies at 16 codegen units (kept) | `lto = true`, 1 codegen unit (cube-tui) |
|---|---|---|
| 2x2 | 8.59 | 8.12 |
| 3x3 | 0.48 | 0.56 |
| 4x4 | 36.69 | 43.36 |
| FTO | 60.69 | 81.53 |
| Square-1 | 6.42 | 5.43 |

| full `cube` build | thin (kept) | fat |
|---|---|---|
| Clean build time | 3 min 47 s | 8 min 3 s |
| `cube.exe` size | 27.3 MB | 24.0 MB |

Fat LTO shows no consistent solver gain, and scrambles are prepared ahead anyway,
while it doubles the build time. The current profile stays.
