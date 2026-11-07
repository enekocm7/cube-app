//! GUI-free core of cube: the solve model, statistics, the timer state machine,
//! scramble generation and persistence.
//!
//! The on-disk formats are shared with [cube-tui](https://github.com/enekocm7/cube-tui),
//! so both applications read and write the same solve history.

pub mod bests;
pub mod blind_stats;
pub mod charts;
pub mod cstimer;
pub mod dialog_keys;
pub mod dialogs;
pub mod event;
pub mod format;
pub mod goals;
pub mod history;
pub mod history_writer;
pub mod keybind;
pub mod library;
pub mod manual_entry;
pub mod merge;
pub mod multi_blind;
pub mod panels;
pub mod persistence;
pub mod scramble;
pub mod search;
pub mod settings;
pub mod settings_category;
pub mod share_card;
pub mod share_link;
pub mod solve;
mod solve_extras;
pub mod stats;
pub mod tags;
pub mod theme;
pub mod timer;
pub mod trainer;
pub mod validation;
pub mod window_state;

pub use event::WcaEvent;
pub use goals::{Goal, GoalError, GoalKind, GoalProgress, Measure, Period, SessionAnchor};
pub use history::History;
pub use keybind::{Action, BindingError, KeyBinding, KeyPress, Keybinds};
pub use manual_entry::{ManualEntry, ManualError};
pub use merge::MergeSummary;
pub use settings::{Settings, UpdateChannel, UpdateFrequency, UpdateSettings};
pub use settings_category::SettingsCategory;
pub use solve::{Modifier, Solve, SolveChange, SolveSnapshot};
pub use tags::{TagChange, TagError};
pub use timer::{HardwareState, InspectionCue, Readiness, Timer, TimerOutcome, TimerPhase};
