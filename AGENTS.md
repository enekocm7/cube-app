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
| `crates/cube-updater` | GitHub release check, verified download and self-update |
| `crates/cube-ui` | The GPUI interface |

Dependency direction: `ui -> core`, `ui -> locale`, `ui -> updater`. **Only
`cube-ui` may depend on GPUI / `gpui-kit`**; keep everything else GUI-free so
it builds and tests quickly. Put reusable logic in `cube-core`, not in the UI.

## Building, running and testing

Run commands from the repo root. This repo uses plain `cargo` (there are no
cargo aliases). CI builds on stable, so do not use nightly-only features.

```sh
# Fast GUI-free tests (core logic, locales, updater)
cargo test -p cube-core -p cube-locale -p cube-updater

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
- When a change affects scramble speed, rerun
  `cargo run --release -p cube-core --example scramble_bench` and update
  `docs/scramble-performance.md`.
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

## Keyboard support

**cube must stay fully controllable from the keyboard.** The mouse is a
convenience, never the only way to do something. That includes the timer,
scrambles, solve navigation and editing, penalties, events, sessions, stats,
settings, import/export dialogs and help.

- Every primary action needs an `Action` in `cube-core::keybind`, a default in
  `Keybinds::default`, a row in the `F1` keyboard shortcuts panel, and an entry
  in `docs/keybinds.md`. That panel is also the editor: keep its pencil, trash
  and capture controls reachable with `Tab` and activatable with `Enter` or
  `Space`; do not make the mouse the only way to reach a control.
- Every dialog routes Enter through `cube_ui::dialog_submit`
  (`submit_on_enter` or `close_on_enter`), never a bare `on_ok`, and shares one
  `Submit` closure between Enter and its footer button. That closure closes the
  dialog itself. Key bindings fire before a focused control sees the key, so
  the helper is what lets Enter on a focused Cancel button cancel.
- Bindings are validated before they are saved: the key must be understood, it
  must not already belong to another action, the timer key must be unmodified,
  and the timer key cannot be removed. Show the user a specific error rather
  than silently ignoring an invalid change.
- All actions except the timer can be unbound. Persist a removal explicitly
  (`""` or `"none"` in `[keybindings]`) so a copied default does not return.
- Keep `docs/keybinds.md` and the README shortcut table in sync with the
  defaults whenever you add or change a binding.
- Parsing, matching and validation belong in `cube-core::keybind` and need
  tests there. `cube-ui` only captures a key and renders the result.

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
- **Saving the history** goes through `history_writer::HistoryWriter`, never
  a bare write. Its thread owns the `persistence::HistoryStore`, which merges
  what cube-tui wrote (`cube-core::merge`), keeps backups, and refuses to
  overwrite a file it can't read. cube-tui rewrites the whole file, so a
  solve missing from it proves nothing: a merge must never let the file
  remove a solve or a session. A merge reaches the UI as a `Merge` it adopts
  only once no attempt or dialog is relying on solve positions.
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
- **Scrolling regions** use `cube-ui`'s `scroll_fade` helpers, never a bare
  `overflow_y_scrollbar`/`overflow_y_scroll`, so every region fades the edge
  past which content continues:
  - `FadeScroll::new(id, background)` for a column of items. It owns the
    scroll handle, the scrollbar and the edge fades, and takes the same
    styles and children as a `v_flex`. Give each one a unique id, pass the
    colour of the surface behind it (usually `cx.theme().background`), and
    call `.nested()` when it sits inside another scroller so the wheel stays
    in the inner region until it reaches an edge.
  - `fading_scroll(list, &handle, background)` for a list that already tracks
    its own handle, such as `uniform_list`. It returns a `Div`; size it
    (`size_full`, `flex_1`, …) so the list has a viewport, and add the
    scrollbar outside the wrapper.
- **Dialogs and panels size themselves from the window**, never with
  hard-coded widths or body heights:
  - Dialogs use `cube_core::dialogs::DialogSize` (`COMPACT`, `STANDARD`,
    `FORM`, `WIDE`) with `crate::dialog_size::dialog_width` for `.w(...)` and
    `crate::dialog_size::dialog_body_height` for the scrolling body's
    `max_h`. Add a size constant there when a new dialog needs one, and keep
    its body scrollable.
  - Side panels go through `cube_core::panels` (`fit_panels`,
    `clamp_panel_width`, the `DEFAULT_*`/`MIN_*` values) and
    `cube_ui::panel_resize`; don't measure or hard-code panel widths in a
    view.
- Timer/WCA behavior (inspection, hold-to-start, `+2`/DNF) lives in
  `cube-core::timer`; the UI only renders phases. Don't duplicate the rules
  in `cube-ui`.
- Lints are enforced at the workspace level: `unsafe_code = "forbid"` and
  pedantic clippy as warnings. No `unsafe`, and no new clippy warnings.
- Add tests for new logic in the GUI-free crates (`cube-core`,
  `cube-locale`); `cube-ui` has no test harness, so keep logic out of it.
- Update `README.md` when you add or change a user-visible feature, a
  setting, or a key binding. When a key binding changes, update
  `docs/keybinds.md` as well: it is the complete keyboard reference.
  Keep README entries short and plain: one line per feature or shortcut,
  saying what it is and not how it works. Do not list examples, key
  sequences, edge cases or behaviour details there; those belong in
  `docs/keybinds.md` or the code.
