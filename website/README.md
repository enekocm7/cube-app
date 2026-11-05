# cube website (Astro)

The project homepage and docs. Built with [Astro](https://astro.build/) (static output, with no framework JS).

## Layout

| Path | What it is |
|---|---|
| `src/pages/index.astro` | Landing page: working timer, feature face, screenshots, interactive keyboard, data, download |
| `src/pages/docs/[...slug].astro` | Renders every `src/content/docs/*.md` file |
| `src/content/docs/*.md` | **All documentation lives here as markdown** — install, timer, shortcuts, sessions, stats, settings, data, themes, bluetooth, updates, FAQ, scramble performance |
| `src/layouts/Base.astro` | Head tags (canonical, Open Graph, Twitter), header/nav, footer, mobile menu |
| `src/layouts/Docs.astro` | Docs sidebar with filter, prev/next pager |
| `src/scripts/` | Client code. Pages swap without a reload, so `page.ts` sets everything up on `astro:page-load` |
| `../fonts/` | Cube Mono, shared with the app: JetBrains Mono with a plain zero, under the OFL |
| `src/lib/cube.ts` | 3x3x3 simulator behind the timer's scramble net |
| `src/lib/keys.ts` | Default shortcuts for the landing page keyboard. Keep in step with `src/content/docs/keybinds.md` |
| `src/styles/global.css` | All styles. Colors come from a cube's stickers and a stackmat's lights |
| `public/` | `logo.svg`, `screenshots/*.webp` (real app captures, 1462x940), `screenshots/timer.png` (social preview) |

## Commands

From this folder (`website/`):

```sh
bun install
bun run dev      # local preview with hot reload
bun run build    # static output in website/dist/
bun run preview  # serve the built site
```

The GitHub Pages workflow (`../.github/workflows/pages.yml`) installs, builds, and deploys `website/dist/` on pushes to `master` touching `website/`, `fonts/`, or `assets/logo.svg`.

## Editing docs

Add a markdown file under `src/content/docs/` with frontmatter:

```md
---
title: My guide
description: One sentence for the cards and meta tags.
section: Start here
order: 3
---
```

Sections used: `Start here`, `Timing`, `History`, `Reference`. The sidebar, filter, and prev/next links update automatically.

## Replacing screenshots

- `public/screenshots/` holds real app captures. `timer.png` is the social preview.
