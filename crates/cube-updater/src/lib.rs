//! In-app updater for cube.
//!
//! All items are re-exported at the crate root, so callers use
//! `cube_updater::fetch_latest()` and friends.

mod updater;

pub use updater::*;
