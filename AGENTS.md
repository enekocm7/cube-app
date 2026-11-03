# AGENTS.md

Guidance for AI coding agents (and humans) working in this repository.

## What this is

cube is a desktop timer and session manager for speedcubing, written in Rust
(edition 2024) with GPUI Kit. It is the graphical sibling of
[cube-tui](https://github.com/enekocm7/cube-tui): both apps read and write the
same solve history, so a save from one must keep working in the other. See
[README.md](README.md) for the user-facing feature list.

## Workspace layout

| Crate | Contents |
|---|---|
| `cube-gui` (root) | The `cube` binary (`src/main.rs`) and icon embedding (`build.rs`) |
| `crates/cube-core` | Solves, statistics, the timer state machine, scrambles and persistence, with no GUI dependency |
| `crates/cube-locale` | Translated strings (`en.rs`, `es.rs`) |
| `crates/cube-ui` | The GPUI interface |

Dependency direction: `ui -> core`, `ui -> locale`. **Only `cube-ui` may
depend on GPUI / `gpui-kit`**; keep everything else GUI-free so it builds and
tests quickly. Put reusable logic in `cube-core`, not in the UI.

## Building, running and testing

Run commands from the repo root. This repo uses plain `cargo` (there are no
cargo aliases). CI builds on stable, so do not use nightly-only features.

```sh
# Fast GUI-free tests (core logic, locales)
cargo test -p cube-core -p cube-locale

# Full test suite
cargo test --workspace

# Type-check everything, including tests
cargo check --workspace --all-targets

# Run the app
cargo run --release
```

Before opening a PR, at minimum `cargo test --workspace` and
`cargo check --workspace --all-targets` must pass. Also run `cargo fmt --all`
and `cargo clippy --workspace --all-targets -- -D warnings` and fix new
warnings in code you touched.

Notes:

- `.cargo/config.toml` points the Windows MSVC target at `rust-lld` for
  faster GPUI-sized links; leave it alone.
- Scramble solvers are slow unoptimized, so the dev profile raises opt-levels
  (`tnoodle-rs` at 3, other deps at 2). Don't "simplify" those profiles away.
- Tests must never touch real user data: `CUBE_DATA_DIR` overrides the data
  directory, and debug builds use a `debug` subfolder automatically. Follow
  the existing `temporary_path()` pattern in `persistence.rs` tests.

## Pull requests

- **Always say which issue the PR closes. This is required, not optional.**
  Put a GitHub closing keyword on its own line in the PR description, e.g.
  `Closes #66` (one line per issue), so merging the PR auto-closes and links
  the issue. Without it the issue stays open and the work is untraceable.
  If there genuinely is no issue, say so explicitly in the description
  (e.g. `No issue: typo fix`), don't just omit the line.
- Title: short imperative summary followed by the issue number, e.g.
  `Add inspection toggle to settings (#66)`.
- Description sections: `## What changed` (bullets) and `## Tests` (the exact
  commands you ran and their result).
- Branch off `main`; never commit directly to `main`.

## Code conventions

- **Shared save format.** `times.json` is cube-tui's file and this app reads
  and writes it (`cube-core::persistence`). Fields may be added but never
  renamed or removed. New optional solve/session fields must be `#[serde(default)]`
  (plus `skip_serializing_if` for empty strings/vecs, matching the existing
  style) so old saves load and new saves stay minimal. The
  `reads_files_written_by_cube_tui` and `writes_the_same_shape_it_reads`
  tests pin this down: keep them passing and extend them when you change the
  shape.
- **Never destroy user data.** A history file that exists but can't be read
  is an error, never an empty history (treating it as empty would let the
  next save overwrite it). The app turns saving off in that case; preserve
  that behavior. Writes go through the atomic sibling-temp-file helper.
- **User-facing strings** go in `cube-locale` as fields on `Strings`, never
  hard-coded in the UI. A missing translation is a compile error by design:
  add the field, fill it in `en.rs` **and** `es.rs`, and keep `{placeholder}`
  markers consistent across languages (the
  `every_language_keeps_the_placeholders` test enforces this). Placeholders
  are filled with `str::replace` by the caller.
- **Settings** (`cube-core::settings`, stored in `cube-gui.toml`, separate
  from cube-tui's `config.toml`) need a sensible default and must keep older
  files loading: `#[serde(default)]` on the struct, tolerate missing and
  unknown keys (see `missing_and_unknown_keys_are_tolerated`). Never share
  the file with cube-tui.
- Timer/WCA behavior (inspection, hold-to-start, `+2`/DNF) lives in
  `cube-core::timer`; the UI only renders phases. Don't duplicate the rules
  in `cube-ui`.
- Lints are enforced at the workspace level: `unsafe_code = "forbid"` and
  pedantic clippy as warnings. No `unsafe`, and no new clippy warnings.
- Add tests for new logic in the GUI-free crates (`cube-core`,
  `cube-locale`); `cube-ui` has no test harness, so keep logic out of it.
- Update `README.md` when you add or change a user-visible feature, a
  setting, or a key binding.
