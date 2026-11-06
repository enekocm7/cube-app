//! The puzzles a solve can belong to.

use serde::{Deserialize, Serialize};

/// A timed puzzle. Variant names are part of the save format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum WcaEvent {
    Cube2x2,
    Cube3x3,
    /// 3x3x3 solved one-handed. Uses ordinary 3x3x3 scrambles.
    Cube3x3OneHanded,
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
    pub const ALL: [Self; 13] = [
        Self::Cube2x2,
        Self::Cube3x3,
        Self::Cube3x3OneHanded,
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
            Self::Cube3x3OneHanded => "3x3x3 OH",
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

/// Infers the most likely event from a scramble, as cube-tui does when it
/// imports csTimer files that record no event.
pub fn classify_event(scramble: &str) -> WcaEvent {
    let text = scramble.trim();
    if text.is_empty() {
        return WcaEvent::Cube3x3;
    }

    if text.contains('(') || text.contains('/') {
        return WcaEvent::Square1;
    }

    if text.contains("R++")
        || text.contains("R--")
        || text.contains("D++")
        || text.contains("D--")
        || text.contains('\n')
    {
        return WcaEvent::Megaminx;
    }

    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.iter().any(|token| is_clock_token(token)) {
        return WcaEvent::Clock;
    }

    let move_count = tokens.len();
    let bases: Vec<&str> = tokens.iter().map(|token| base_move(token)).collect();
    if bases
        .iter()
        .all(|base| matches!(*base, "R" | "L" | "U" | "B"))
    {
        return if move_count <= 10 {
            WcaEvent::Skewb
        } else {
            WcaEvent::Pyraminx
        };
    }

    let has_three_wide = bases
        .iter()
        .any(|base| matches!(*base, "3Rw" | "3Lw" | "3Uw" | "3Dw" | "3Fw" | "3Bw"));
    if has_three_wide {
        return if move_count >= 90 {
            WcaEvent::Cube7x7
        } else {
            WcaEvent::Cube6x6
        };
    }

    let has_wide = bases
        .iter()
        .any(|base| matches!(*base, "Rw" | "Lw" | "Uw" | "Dw" | "Fw" | "Bw"));
    if has_wide {
        return if move_count >= 50 {
            WcaEvent::Cube5x5
        } else {
            WcaEvent::Cube4x4
        };
    }

    if bases.iter().all(|base| matches!(*base, "R" | "U" | "F")) {
        return WcaEvent::Cube2x2;
    }

    WcaEvent::Cube3x3
}

/// Removes a move's amount or direction suffix for classification.
fn base_move(token: &str) -> &str {
    token
        .strip_suffix('2')
        .unwrap_or_else(|| token.strip_suffix('\'').map_or(token, |stripped| stripped))
}

/// Returns whether a token resembles a clock dial turn.
///
/// csTimer writes clock amounts after the sign (`UR3+`); the ported check also
/// accepts the sign first (`UR+3`) so both notations classify as Clock.
fn is_clock_token(token: &str) -> bool {
    const POSITIONS: [&str; 9] = ["UR", "DR", "DL", "UL", "U", "R", "D", "L", "ALL"];
    for position in POSITIONS {
        if let Some(rest) = token.strip_prefix(position) {
            if rest.len() < 2 {
                continue;
            }
            let amount = rest
                .strip_suffix(['+', '-'])
                .or_else(|| rest.strip_prefix(['+', '-']));
            if amount.is_some_and(|amount| amount.chars().all(|c| c.is_ascii_digit())) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{WcaEvent, classify_event};

    #[test]
    fn classifies_every_event_from_its_scramble() {
        assert_eq!(classify_event(""), WcaEvent::Cube3x3);
        assert_eq!(classify_event("R U F' R2"), WcaEvent::Cube2x2);
        assert_eq!(
            classify_event("R U R' U' F2 D L B' R2 F' U2 B2 L' D' F R U"),
            WcaEvent::Cube3x3
        );
        assert_eq!(
            classify_event("Rw U2 Fw' L2 D Rw2 B' Uw R' Fw2 L U2 Rw B2 Uw' D' R2 F L2"),
            WcaEvent::Cube4x4
        );
        assert_eq!(
            classify_event(
                "Rw U2 Fw' L2 D Rw2 B' Uw R' Fw2 L U2 Rw B2 Uw' D' R2 F L2 \
                 Rw U2 Fw' L2 D Rw2 B' Uw R' Fw2 L U2 Rw B2 Uw' D' R2 F L2 \
                 Rw U2 Fw' L2 D Rw2 B' Uw R' Fw2 L U2 Rw B2 Uw' D' R2 F L2"
            ),
            WcaEvent::Cube5x5
        );
        assert_eq!(
            classify_event("3Rw U2 3Fw' L2 D 3Rw2 B' Uw R' Fw2 L U2 Rw B2 Uw' D' R2 F L2"),
            WcaEvent::Cube6x6
        );
        assert_eq!(
            classify_event(
                "3Rw U2 3Fw' L2 D 3Rw2 B' Uw R' Fw2 L U2 Rw B2 Uw' D' R2 F L2 \
                 3Rw U2 3Fw' L2 D 3Rw2 B' Uw R' Fw2 L U2 Rw B2 Uw' D' R2 F L2 \
                 3Rw U2 3Fw' L2 D 3Rw2 B' Uw R' Fw2 L U2 Rw B2 Uw' D' R2 F L2 \
                 3Rw U2 3Fw' L2 D 3Rw2 B' Uw R' Fw2 L U2 Rw B2 Uw' D' R2 F L2 \
                 3Rw U2 3Fw' L2 D 3Rw2 B' Uw R' Fw2 L U2 Rw B2 Uw' D' R2 F L2"
            ),
            WcaEvent::Cube7x7
        );
        assert_eq!(
            classify_event("R++ D-- R++ D++ R-- D-- U"),
            WcaEvent::Megaminx
        );
        assert_eq!(
            classify_event("R U L' B U' L R' B' U' L R"),
            WcaEvent::Pyraminx
        );
        assert_eq!(classify_event("R U L' B U'"), WcaEvent::Skewb);
        assert_eq!(
            classify_event("(1,0) / (-1,-1) / (0,1) / (0,-1)"),
            WcaEvent::Square1
        );
        assert_eq!(
            classify_event("UR3+ DR4- DL1+ UL2- U5+ R6- D0+ L1- ALL2+ y2 UR1+"),
            WcaEvent::Clock
        );
    }

    #[test]
    fn long_scrambles_with_only_skewb_moves_are_pyraminx() {
        let scramble = "R U L B R' U' L' B' R U L B R'";
        assert_eq!(classify_event(scramble), WcaEvent::Pyraminx);
    }

    #[test]
    fn unknown_notation_falls_back_to_three_by_three() {
        assert_eq!(
            classify_event("U R F L B BL D BR U' R' F'"),
            WcaEvent::Cube3x3
        );
    }
}
