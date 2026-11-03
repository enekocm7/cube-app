//! The puzzles a solve can belong to.

use serde::{Deserialize, Serialize};

/// A timed puzzle. Variant names are part of the save format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WcaEvent {
    Cube2x2,
    Cube3x3,
    Cube4x4,
    Cube5x5,
    Cube6x6,
    Cube7x7,
    Megaminx,
    Pyraminx,
    Fto,
    Skewb,
    Square1,
    Clock,
}

impl WcaEvent {
    /// Every event, in navigation order.
    pub const ALL: [Self; 12] = [
        Self::Cube2x2,
        Self::Cube3x3,
        Self::Cube4x4,
        Self::Cube5x5,
        Self::Cube6x6,
        Self::Cube7x7,
        Self::Megaminx,
        Self::Pyraminx,
        Self::Fto,
        Self::Skewb,
        Self::Square1,
        Self::Clock,
    ];

    /// Returns the puzzle's display name, which is the same in every language.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Cube2x2 => "2x2x2",
            Self::Cube3x3 => "3x3x3",
            Self::Cube4x4 => "4x4x4",
            Self::Cube5x5 => "5x5x5",
            Self::Cube6x6 => "6x6x6",
            Self::Cube7x7 => "7x7x7",
            Self::Megaminx => "Megaminx",
            Self::Pyraminx => "Pyraminx",
            Self::Fto => "FTO",
            Self::Skewb => "Skewb",
            Self::Square1 => "Square-1",
            Self::Clock => "Clock",
        }
    }
}
