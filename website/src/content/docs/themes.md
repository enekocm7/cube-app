---
title: Themes
description: Share cube-tui theme files, edit them with a color picker, and make your own.
section: Reference
order: 3
---

cube reads the same theme files as [cube-tui](https://github.com/enekocm7/cube-tui), so a theme works in both apps. They live in the `themes` folder inside your [data folder](/cube-gui/docs/data/).

## Using themes

In **Settings → Appearance**, every valid theme is listed. Pick one to apply it, use the pencil to edit it, the trash can to delete it, or **New theme** to create one. Themes you drop into the folder appear the next time the dialog opens.

## The format

A theme is a TOML file of six-digit colors:

```toml
background = "#000000"
border = "#FFFFFF"
scramble = "#FFFFFF"
selection = "#3399FF"
selection_text = "#000000"
text = "#FFFFFF"
accent = "#FFD700"
```

- `accent` is optional and defaults to gold, so older cube-tui themes keep working.
- cube creates `default.toml` the first time it makes the themes folder. A deleted theme is not recreated.
- Theme files store opaque colors: an alpha typed into an RGBA/HSLA field is ignored when saving.

## The editor

- A color picker per color, a text field switching between Hex, RGB, RGBA, HSL and HSLA, and a live preview.
- Rename a theme by changing its name — cube saves it as `<name>.toml`.
