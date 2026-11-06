---
title: Stats and charts
description: Averages, a heat-mapped table, trends, calendars, and all-time bests.
section: History
order: 2
---

## The right panel

The timer screen's right panel compares **Current** and **Best** for the single and every rolling average: mo3, ao5, ao12, ao50, ao100. Click (or `Tab` + `Enter` on) any value to open the solves behind it.

## Goals (`Shift + G`)

The tick icon in the title bar, beside the Bluetooth one, opens your goals. Each goal is for one event and counts every session unless you limit it to one.

| Goal | Reached when |
|---|---|
| **Volume** | You do that many solves in a day or a week. DNFs count: it measures practice |
| **Target time** | Your best single or average beats the time. Equal is not enough, and averages stay inside a session |
| **Streak** | You do that many solves a day for that many days in a row. Today only adds to a streak: it ends when a whole day is missed |

- Days and weeks follow your computer's local time, and weeks start on Monday. Solves with no date count towards target times but not towards days.
- Progress always comes from your solves, so it stays right after editing, deleting or restoring one, or after a merge from cube-tui.
- The timer screen's right panel shows the goals for the current event and session, and a toast tells you when you reach one.
- A goal limited to a session stays with it when the session is renamed. If you delete the session the goal is paused, and undoing the deletion brings it back.

## Stats screen (`A`)

A table of every solve's rolling averages:

- The header shows each column's best. Click it to jump to that result.
- Tiles are heat-mapped: green for fast, orange for slow, unshaded in between.
- The best tile of each column is tinted green.
- `Left` / `Right` picks the highlighted average column; `O` opens it, `Shift + O` opens its best, `C` copies it as text to share, `Shift + K` creates an image of it.
- `L`, or the menu above the table, keeps only the solves with a tag. Averages then run over those solves alone.

## Charts screen (`G`)

Five views, cycled with `V`:

- **Trends** — your times plus ao5, ao12 and ao100 lines, personal bests, and DNF marks.
- **Calendar** — solves per day. Arrow keys move the selected day on the calendar view.
- **Compare** — sessions side by side.
- **Tags** — two tags side by side: solves, best single, mean and best ao5, ao12 and ao100. With all sessions on, each tag's solves count as one run by date.
- **All-time bests** — your best single and averages per event.

Filters:

| Key | What it does |
|---|---|
| `F` | Cycle the event filter |
| `R` | Cycle the range: all, last 50/100/500, last 7/30/90/365 days |
| `Shift + R` | Custom range as `YYYY-MM-DD` |
| `W` | One session or all merged by date |
| `L` | Cycle the tag filter, shared with the stats screen. On Tags it sets the left side |
| `Shift + L` | On Tags, cycle the right side |

On Trends, the mouse wheel zooms around the cursor (`Z` / `Shift + Z` / `X` do the same from the keyboard, hold to keep zooming). `Shift` + wheel pans while zoomed. Changing a filter resets the zoom.
