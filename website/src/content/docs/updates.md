---
title: Updates
description: Release channels, check frequency, verified downloads, and portable vs installed updates.
section: Reference
order: 4
---

cube checks GitHub for new releases and can update itself. It always waits for you to accept, and verifies the download against the release's SHA-256 checksums before installing anything.

## Settings

Under **Settings → Updates**:

| Option | Default | What it does |
|---|---|---|
| Automatic checks | On | Check in the background |
| Channel | Stable | Offer pre-releases too |
| Frequency | Every start | Or at most daily / weekly |
| Check now | — | Run a manual check |
| Skipped version | — | Clear a version you chose to skip |

## How installs apply

- **Installed builds** (Windows installer) update silently through the installer flow.
- **Portable builds** swap the executable and restart.

## Channels and releases

- Stable: finished releases only.
- Pre-release: also offers release candidates and betas.
- Every release publishes `SHA256SUMS` files alongside the archives; the updater refuses a download whose hash mismatches.

See [Install](/cube-app/docs/install/) for the per-platform artifacts, and the [releases page](https://github.com/enekocm7/cube-app/releases) for notes.
