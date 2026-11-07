//! The 3x3x3 sets the rest of the trainer names. Their cases are in the
//! library's data, like every other set.

use std::sync::LazyLock;

use super::AlgSet;
use crate::library::find_set;

fn known_set(id: &str) -> &'static AlgSet {
    find_set(id).unwrap_or_else(|| panic!("The library has no {id} set"))
}

/// The 21 cases of permuting the last layer.
pub static PLL: LazyLock<&'static AlgSet> = LazyLock::new(|| known_set("pll"));

/// The 57 cases of orienting the last layer.
pub static OLL: LazyLock<&'static AlgSet> = LazyLock::new(|| known_set("oll"));
