//! GUI-free core of cube: the solve model, statistics, the timer state machine,
//! scramble generation and persistence.
//!
//! The on-disk formats are shared with [cube-tui](https://github.com/enekocm7/cube-tui),
//! so both applications read and write the same solve history.

pub mod cstimer;
pub mod event;
pub mod format;
pub mod history;
pub mod keybind;
pub mod persistence;
pub mod scramble;
pub mod settings;
pub mod solve;
pub mod stats;
pub mod theme;
pub mod timer;

pub use event::WcaEvent;
pub use history::History;
pub use keybind::{Action, BindingError, KeyBinding, KeyPress, Keybinds};
pub use settings::{Settings, UpdateChannel, UpdateFrequency, UpdateSettings};
pub use solve::{Modifier, Solve, SolveChange, SolveSnapshot};
pub use timer::{InspectionCue, Readiness, Timer, TimerOutcome, TimerPhase};
