---
title: Timer and inspection
description: Hold-to-start, WCA inspection, manual entry, and the scramble preview.
section: Timing
order: 1
---

## The basic loop

1. A scramble is already waiting for your current event.
2. Press and **hold `Space`** — the digits turn red while arming, green when ready.
3. Release to start, solve, then press **any key** to stop.
4. The solve lands in the list with its scramble, event, and date.

With inspection on (the default), the first `Space` press starts a 15-second inspection countdown instead of the solve. The second hold starts the solve itself.

## Hold colors

They work like a stackmat:

| Color | Meaning |
|---|---|
| White | Idle — hold `Space` to arm |
| Red | Arming — keep holding |
| Green | Ready — release to start |

Touching the digits with the mouse does the same thing while **Timer → Touch the digits** is on (off by default).

## WCA inspection

- Counts up to 15 seconds. Going over applies `+2` or DNF automatically, following WCA rules.
- Optional audio cues at 8 s and 12 s: turn on **Timer → Inspection audio** in settings.
- Turn inspection itself on or off with `I`, or in **Timer** settings.
- `Esc` cancels the attempt without recording it — during inspection or during the solve.

## Scrambles

- Official random-state scrambles for all 12 events, generated with `tnoodle-rs`.
- Scrambles are prepared ahead of time per event, so switching events is instant.
- **Font size → Scramble size** picks Small, Normal, Large, Largest, or a Custom size in points (6–120).
- The puzzle preview under the timer draws the scrambled state. Toggle it with `P` or **Appearance → Show the scramble preview**.
- **Timer → Shorter 3x3x3 scrambles** trades speed for shorter scrambles (slower to generate).

Supported events: 2x2x2, 3x3x3, 4x4x4, 5x5x5, 6x6x6, 7x7x7, Megaminx, Pyraminx, FTO, Skewb, Square-1, Clock.

## Manual mode

Press `M` to type in times taken on another timer:

- Digits, `:` and `.` build the time (`8.98`, `1:32.49`). `Backspace` deletes, `Ctrl + V` pastes.
- `Enter` records the solve against the current scramble and moves on, staying in manual mode.
- `Enter` with nothing typed opens the selected solve's details instead.
- Press `M` again to return to the live timer.

## Bluetooth timers

Connect a GAN timer and it records solves on its own: press `B`, pick your timer with `Up`/`Down`, `Enter` to connect. While connected, the timer key and digits follow the hardware timer and inspection is skipped. See [Bluetooth](/cube-gui/docs/bluetooth/).
