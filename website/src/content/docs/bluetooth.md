---
title: Bluetooth timers
description: Connect a GAN timer over Bluetooth and let the hardware start and stop solves.
section: Timing
order: 3
---

## Connecting

1. Turn your GAN timer on.
2. Press `B` (or the Bluetooth icon in the title bar).
3. Pick your timer with `Up` / `Down`, `Enter` to connect. Each row is also a button — `Tab` to it, `Enter` or `Space` to press it.
4. The dialog closes once connected. Closing it while connecting leaves the attempt running, and the title-bar icon shows the result.

Press `B` again to see the connected timer; `Enter` disconnects it. While connecting, `Enter` cancels.

## While connected

- The timer key and the digits do nothing: the solve starts and stops on the hardware, and the digits follow it.
- Inspection is skipped.
- `Esc` still cancels the attempt so it is never recorded when the hardware stops.
- Every other shortcut keeps working.

## Troubleshooting

- Make sure no other app (including cube-tui or a phone app) is already connected to the timer — Bluetooth timers take one connection.
- Move closer and try again; walls and USB-3 hubs are classic interferers.
- If the timer appears but connecting stalls, cancel with `Enter` and reconnect.
