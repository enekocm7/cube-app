//! Algorithm notation, reduced to turns of the six outer faces.
//!
//! `min2phase` only understands outer face turns and never reports an error:
//! it drops rotations and misreads `r'` or `M'` as other moves. So every
//! algorithm goes through [`parse`] first, which keeps the centers where they
//! are by rewriting wide turns, slices and rotations as outer turns seen from
//! a turning frame of reference. What is left is the same cube, up to the
//! rotation of the whole, which a solver of the case never needs.

use std::fmt;

/// One of the six outer faces, in the order the frame below indexes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    U,
    R,
    F,
    D,
    L,
    B,
}

impl Face {
    const fn opposite(self) -> Self {
        match self {
            Self::U => Self::D,
            Self::R => Self::L,
            Self::F => Self::B,
            Self::D => Self::U,
            Self::L => Self::R,
            Self::B => Self::F,
        }
    }

    const fn letter(self) -> char {
        match self {
            Self::U => 'U',
            Self::R => 'R',
            Self::F => 'F',
            Self::D => 'D',
            Self::L => 'L',
            Self::B => 'B',
        }
    }

    pub(super) const fn from_letter(letter: char) -> Option<Self> {
        match letter {
            'U' => Some(Self::U),
            'R' => Some(Self::R),
            'F' => Some(Self::F),
            'D' => Some(Self::D),
            'L' => Some(Self::L),
            'B' => Some(Self::B),
            _ => None,
        }
    }
}

/// A clockwise turn of an outer face by one to three quarters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Turn {
    pub face: Face,
    pub quarters: u8,
}

impl Turn {
    /// A turn of `quarters` quarters, reduced to less than a full turn.
    pub const fn new(face: Face, quarters: u8) -> Self {
        Self {
            face,
            quarters: quarters % 4,
        }
    }
}

impl fmt::Display for Turn {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let suffix = match self.quarters {
            2 => "2",
            3 => "'",
            _ => "",
        };
        write!(formatter, "{}{suffix}", self.face.letter())
    }
}

/// Why an algorithm could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AlgError {
    /// The text holds no moves.
    Empty,
    /// This part of the text is not a move.
    InvalidMove(String),
}

/// A whole-cube rotation, named by its axis.
#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
    Z,
}

/// What a move letter does: its outer turns, each with whether it goes against
/// the move's own direction, then the rotation that follows, likewise.
type Expansion = (&'static [(Face, bool)], Option<(Axis, bool)>);

/// Rewrites a move as outer turns and a rotation. A wide turn is the opposite
/// face plus a rotation, and a slice is its two faces plus a rotation.
fn expansion(letter: char, wide: bool) -> Option<Expansion> {
    if wide {
        return match letter.to_ascii_uppercase() {
            'R' => Some((&[(Face::L, false)], Some((Axis::X, false)))),
            'L' => Some((&[(Face::R, false)], Some((Axis::X, true)))),
            'U' => Some((&[(Face::D, false)], Some((Axis::Y, false)))),
            'D' => Some((&[(Face::U, false)], Some((Axis::Y, true)))),
            'F' => Some((&[(Face::B, false)], Some((Axis::Z, false)))),
            'B' => Some((&[(Face::F, false)], Some((Axis::Z, true)))),
            _ => None,
        };
    }
    match letter {
        'U' => Some((&[(Face::U, false)], None)),
        'R' => Some((&[(Face::R, false)], None)),
        'F' => Some((&[(Face::F, false)], None)),
        'D' => Some((&[(Face::D, false)], None)),
        'L' => Some((&[(Face::L, false)], None)),
        'B' => Some((&[(Face::B, false)], None)),
        'M' => Some((&[(Face::R, false), (Face::L, true)], Some((Axis::X, true)))),
        'E' => Some((&[(Face::U, false), (Face::D, true)], Some((Axis::Y, true)))),
        'S' => Some((&[(Face::F, true), (Face::B, false)], Some((Axis::Z, false)))),
        'x' | 'X' => Some((&[], Some((Axis::X, false)))),
        'y' | 'Y' => Some((&[], Some((Axis::Y, false)))),
        'z' | 'Z' => Some((&[], Some((Axis::Z, false)))),
        _ => None,
    }
}

/// Which outer face sits where the algorithm thinks each face is.
#[derive(Clone, Copy)]
struct Frame([Face; 6]);

impl Frame {
    const IDENTITY: Self = Self([Face::U, Face::R, Face::F, Face::D, Face::L, Face::B]);

    const fn get(self, face: Face) -> Face {
        self.0[face as usize]
    }

    /// Turns the frame a quarter clockwise around `axis`: each face of the
    /// cycle takes over from the one after it.
    fn rotate(&mut self, axis: Axis) {
        let cycle = match axis {
            Axis::X => [Face::U, Face::F, Face::D, Face::B],
            Axis::Y => [Face::F, Face::R, Face::B, Face::L],
            Axis::Z => [Face::R, Face::U, Face::L, Face::D],
        };
        let old = *self;
        for (index, face) in cycle.iter().enumerate() {
            self.0[*face as usize] = old.get(cycle[(index + 1) % 4]);
        }
    }
}

/// Reads an algorithm as outer turns. Parentheses and brackets are ignored,
/// and moves may be written with or without spaces between them.
pub fn parse(algorithm: &str) -> Result<Vec<Turn>, AlgError> {
    let is_gap = |letter: char| letter.is_whitespace() || "()[]".contains(letter);
    let mut turns = Vec::new();
    let mut frame = Frame::IDENTITY;
    let mut any = false;
    let mut letters = algorithm.char_indices().peekable();

    while let Some((start, letter)) = letters.next() {
        if is_gap(letter) {
            continue;
        }
        let invalid = || {
            let rest = &algorithm[start..];
            let end = rest.find(is_gap).unwrap_or(rest.len());
            AlgError::InvalidMove(rest[..end].to_owned())
        };
        let mut wide = "urfdlb".contains(letter);
        if letters.peek().is_some_and(|&(_, next)| next == 'w') {
            if !"URFDLB".contains(letter) {
                return Err(invalid());
            }
            letters.next();
            wide = true;
        }
        let (parts, rotation) = expansion(letter, wide).ok_or_else(invalid)?;

        let mut quarters = 1;
        if let Some(&(_, digit)) = letters.peek()
            && let Some(count) = digit.to_digit(10)
        {
            if !(1..=3).contains(&count) {
                return Err(invalid());
            }
            quarters = count as u8;
            letters.next();
        }
        if letters
            .peek()
            .is_some_and(|&(_, next)| matches!(next, '\'' | '’' | '`'))
        {
            quarters = 4 - quarters;
            letters.next();
        }

        any = true;
        for &(face, against) in parts {
            let amount = if against { 4 - quarters } else { quarters };
            turns.push(Turn::new(frame.get(face), amount));
        }
        if let Some((axis, against)) = rotation {
            let amount = if against { 4 - quarters } else { quarters };
            for _ in 0..amount {
                frame.rotate(axis);
            }
        }
    }
    if any { Ok(turns) } else { Err(AlgError::Empty) }
}

/// Merges turns of one face, including those with only the opposite face
/// between them, and drops what cancels out.
pub fn simplify(turns: impl IntoIterator<Item = Turn>) -> Vec<Turn> {
    let mut simplified: Vec<Turn> = Vec::new();
    for turn in turns {
        let count = simplified.len();
        let same_face = |index: usize| simplified[index].face == turn.face;
        let target = if count >= 1 && same_face(count - 1) {
            Some(count - 1)
        } else if count >= 2
            && same_face(count - 2)
            && simplified[count - 1].face == turn.face.opposite()
        {
            Some(count - 2)
        } else {
            None
        };
        match target {
            Some(index) => {
                let quarters = (simplified[index].quarters + turn.quarters) % 4;
                if quarters == 0 {
                    simplified.remove(index);
                } else {
                    simplified[index].quarters = quarters;
                }
            }
            None if turn.quarters != 0 => simplified.push(turn),
            None => {}
        }
    }
    simplified
}

/// Writes turns as the text of a scramble.
pub fn to_text(turns: &[Turn]) -> String {
    let turned = turns.iter().filter(|turn| turn.quarters != 0);
    let texts: Vec<String> = turned.map(ToString::to_string).collect();
    texts.join(" ")
}

/// Reads outer turns as the solver writes them, or `None` if it reported an
/// error instead.
pub(super) fn parse_outer(text: &str) -> Option<Vec<Turn>> {
    text.split_whitespace()
        .map(|token| {
            let mut letters = token.chars();
            let face = Face::from_letter(letters.next()?)?;
            let quarters = match letters.as_str() {
                "" => 1,
                "2" => 2,
                "'" => 3,
                _ => return None,
            };
            Some(Turn::new(face, quarters))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::invert;
    use tnoodle::min2phase::tools::from_scramble;

    fn cube(algorithm: &str) -> String {
        from_scramble(&to_text(&parse(algorithm).unwrap()))
    }

    #[test]
    fn outer_turns_come_through_unchanged() {
        assert_eq!(
            to_text(&parse("R U2 F' (D L) [B2]").unwrap()),
            "R U2 F' D L B2"
        );
    }

    #[test]
    fn spaces_between_moves_are_optional() {
        assert_eq!(to_text(&parse("RU2F'").unwrap()), "R U2 F'");
    }

    #[test]
    fn every_form_of_a_prime_and_a_double_is_read() {
        assert_eq!(to_text(&parse("R2' U’ F3 D1").unwrap()), "R2 U' F' D");
    }

    #[test]
    fn a_wide_turn_is_the_slice_and_the_face_together() {
        assert_eq!(cube("r"), cube("R M'"));
        assert_eq!(cube("Rw"), cube("R M'"));
        assert_eq!(cube("l"), cube("L M"));
        assert_eq!(cube("u"), cube("U E'"));
        assert_eq!(cube("d"), cube("D E"));
        assert_eq!(cube("f"), cube("F S"));
        assert_eq!(cube("b"), cube("B S'"));
    }

    #[test]
    fn a_rotation_moves_what_follows_it() {
        assert_eq!(to_text(&parse("y R").unwrap()), "B");
        assert_eq!(to_text(&parse("y' R").unwrap()), "F");
        assert_eq!(to_text(&parse("x U").unwrap()), "F");
        assert_eq!(to_text(&parse("z U").unwrap()), "L");
        assert_eq!(to_text(&parse("y2 R").unwrap()), "L");
    }

    #[test]
    fn a_rotation_alone_turns_nothing() {
        assert_eq!(parse("x y z'").unwrap(), Vec::new());
    }

    #[test]
    fn four_slices_cancel_out() {
        let solved = from_scramble("");
        assert_eq!(cube("M M M M"), solved);
        assert_eq!(cube("E2 E2"), solved);
        assert_eq!(cube("S S'"), solved);
        assert_eq!(cube("r r r r"), solved);
    }

    #[test]
    fn a_slice_follows_the_frame_it_is_turned_in() {
        assert_eq!(cube("y M y'"), cube("S"));
        assert_eq!(cube("y' M y"), cube("S'"));
    }

    #[test]
    fn invalid_moves_are_named() {
        assert_eq!(
            parse("R U Q2 R'"),
            Err(AlgError::InvalidMove("Q2".to_owned()))
        );
        assert_eq!(parse("R5"), Err(AlgError::InvalidMove("R5".to_owned())));
        assert_eq!(parse("Mw"), Err(AlgError::InvalidMove("Mw".to_owned())));
    }

    #[test]
    fn text_without_moves_is_empty() {
        assert_eq!(parse(""), Err(AlgError::Empty));
        assert_eq!(parse(" ( ) "), Err(AlgError::Empty));
    }

    #[test]
    fn an_inverse_undoes_the_turns() {
        let turns = parse("r U R' F2 M' y L").unwrap();
        let both: Vec<Turn> = turns.iter().copied().chain(invert(&turns)).collect();
        assert_eq!(from_scramble(&to_text(&both)), from_scramble(""));
    }

    #[test]
    fn simplifying_merges_and_cancels() {
        let simplified = |text: &str| to_text(&simplify(parse(text).unwrap()));
        assert_eq!(simplified("R R"), "R2");
        assert_eq!(simplified("R R'"), "");
        assert_eq!(simplified("U R R' U'"), "");
        assert_eq!(simplified("R L R"), "R2 L");
        assert_eq!(simplified("R L R'"), "L");
    }

    #[test]
    fn solver_output_is_read_back() {
        let turns = parse_outer("R U2 F' ").unwrap();
        assert_eq!(to_text(&turns), "R U2 F'");
        assert_eq!(parse_outer("Error 7"), None);
    }
}
