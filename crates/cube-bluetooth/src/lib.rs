//! GAN Bluetooth timers: finding them, connecting and following their state.
//!
//! The Bluetooth work runs on its own background runtime, so callers only see
//! handles with an async `next` that any executor can drive. Dropping a
//! [`Scan`] stops it; dropping a [`Link`] disconnects the timer.

mod client;
mod picker;
mod protocol;

pub use client::{Client, ConnectError, Link, LinkEvent, LinkEvents, Problem, Scan, ScanEvent};
pub use picker::{Device, Picker, ScanError};
