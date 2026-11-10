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

## Personal bests

When a solve beats your best single or ao5, the digits flash in your theme's accent color, and a chime plays if the sound is on. The effect never delays the next attempt: the timer key works straight away.

- A session best gets one flash. An all-time best for the event gets three, and the digits grow a little.
- A best only counts once there are enough earlier solves of that event to beat, 100 by default: in the session for a session best, across all sessions for an all-time best. So a new session stays quiet about its own bests, but still celebrates an all-time one. The session best notifications wait for that many solves in the session too. Change the number with **Solves before a best counts**.
- A solve that sets several bests celebrates once, for the biggest: all-time before session, ao5 before single.
- Ties and DNFs never count, and neither do imports, merges or edits.

Turn the effect and the sound on or off separately in the timer settings. GPUI does not report your system's reduced motion preference, so **Reduce motion** there replaces the flash with a steady color.

## Scrambles

- Official random-state scrambles for all 16 events, generated with `tnoodle-rs`.
- Scrambles are prepared ahead of time per event, so switching events is instant.
- **Appearance → Scramble size** picks Small, Normal, Large, Largest, or a Custom size in points (6–120).
- The puzzle preview under the timer draws the scrambled state. Toggle it with `P` or **Timer → Show the scramble preview**.
- Click the preview, press `Ctrl + P` or turn on **Timer → 3D scramble preview** to draw the puzzle in 3D instead of the flat net, with the same colors. Every event has a 3D view. Turn it with the mouse as in most 3D viewers:
  - Drag to turn it freely in any direction, as far as you like.
  - Hold `Shift` while dragging to keep to one axis: a drag that starts sideways only turns it left and right, and one that starts up or down only tilts it.
  - Drag with the right button, or hold `Alt`, to roll it around the middle like a dial.
  - Clicking it without dragging, or `Ctrl + P`, returns to the flat net.

  From the keyboard, `Ctrl` with the arrow keys turns and tilts it, `Ctrl + Shift + Left` and `Ctrl + Shift + Right` roll it, and `Ctrl + 0` puts it back the way it started.
- **Timer → Shorter 3x3x3 scrambles** trades speed for shorter scrambles (slower to generate).

Supported events: 2x2x2, 3x3x3, 3x3x3 OH (one-handed, with its own stats and 3x3x3 scrambles), 4x4x4, 5x5x5, 6x6x6, 7x7x7, 3x3x3 Blindfolded, 4x4x4 Blindfolded, 5x5x5 Blindfolded, Megaminx, Pyraminx, FTO, Skewb, Square-1, Clock. The blindfolded events work differently: see [Blindfolded events](/cube-app/docs/blindfolded/), which also covers Multi-Blind.

## Manual mode

Press `M` to type in times taken on another timer:

- Digits, `:` and `.` build the time (`8.98`, `1:32.49`). `Backspace` deletes, `Ctrl + V` pastes.
- `Enter` records the solve against the current scramble and moves on, staying in manual mode.
- `Enter` with nothing typed opens the selected solve's details instead.
- Press `M` again to return to the live timer.

## Bluetooth timers

Connect a GAN timer and it records solves on its own: press `B`, pick your timer with `Up`/`Down`, `Enter` to connect. While connected, the timer key and digits follow the hardware timer and inspection is skipped. See [Bluetooth](/cube-app/docs/bluetooth/).
