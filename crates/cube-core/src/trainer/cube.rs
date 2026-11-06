//! What the cases of a 3x3x3 set mean on a cube: checking that an algorithm
//! solves one, and scrambling into one.

use anyhow::{Result, anyhow, bail};
use tnoodle::java::RandomSource;
use tnoodle::min2phase::tools::from_scramble;
use tnoodle::min2phase::{INVERSE_SOLUTION, Search};

use super::deck::bounded;
use super::notation::{self, AlgError, Face, Turn, invert, parse_outer, simplify, to_text};
use super::three_by_three::PLL;
use super::{AlgCase, AlgSet, Step};

/// The longest solution asked of the solver. It finds one this short almost
/// instantly, and it is what `TNoodle` asks of it too.
const MAX_SOLUTION: i32 = 21;

/// How long the solver may look, in probes, before giving up.
const SOLVER_PROBES: i64 = 1_000_000;

/// Random turns a scramble opens with. The solver always gives the same answer
/// for the same state, so with no opening a case would have a handful of
/// scrambles that soon become familiar.
const OPENING_TURNS: usize = 3;

/// Why a custom algorithm cannot be used for a case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CustomAlgError {
    /// It could not be read.
    Notation(AlgError),
    /// It reads, but does not solve the case.
    WrongCase,
}

/// The facelets of a cube after `turns`, in `URFDLB` order.
pub(super) fn facelets(turns: &[Turn]) -> String {
    from_scramble(&to_text(turns))
}

/// A turn of the last layer.
const fn auf(quarters: u8) -> Turn {
    Turn::new(Face::U, quarters)
}

/// Returns the turns that set up `case` from a solved cube.
pub(super) fn setup(case: &AlgCase) -> Result<Vec<Turn>> {
    let turns = notation::parse(case.algorithm)
        .map_err(|error| anyhow!("Invalid algorithm for {}: {error:?}", case.name))?;
    Ok(invert(&turns))
}

/// Returns whether the first two layers are solved.
fn first_two_layers_solved(facelets: &[u8]) -> bool {
    // Each side's lower two rows, and the whole of the bottom face.
    [
        (12, 18, b'R'),
        (21, 27, b'F'),
        (39, 45, b'L'),
        (48, 54, b'B'),
    ]
    .into_iter()
    .chain([(27, 36, b'D')])
    .all(|(start, end, face)| facelets[start..end].iter().all(|&sticker| sticker == face))
}

impl Step {
    /// Returns whether a cube that went through `turns` finished this step.
    fn reached(self, turns: &[Turn]) -> bool {
        match self {
            Self::OrientLastLayer => {
                let cube = facelets(turns);
                let cube = cube.as_bytes();
                first_two_layers_solved(cube) && cube[..9].iter().all(|&sticker| sticker == b'U')
            }
            Self::PermuteLastLayer => {
                let solved = from_scramble("");
                (0..4).any(|quarters| {
                    let mut turns = turns.to_vec();
                    turns.push(auf(quarters));
                    facelets(&turns) == solved
                })
            }
        }
    }

    /// Returns what goes before a case's setup, for variety that leaves the
    /// case as it is: a turn of the last layer, and for orientation any
    /// permutation of it too, since orienting never depends on that.
    fn opening(self, random: &mut dyn RandomSource) -> Result<Vec<Turn>> {
        let mut turns = Vec::new();
        if self == Self::OrientLastLayer {
            turns = setup(&PLL.cases[bounded(random, PLL.cases.len())])?;
        }
        turns.push(random_auf(random));
        Ok(turns)
    }
}

fn random_auf(random: &mut dyn RandomSource) -> Turn {
    auf(bounded(random, 4) as u8)
}

/// Checks that `algorithm` solves `case`, from any angle of the last layer: an
/// algorithm may start with a turn of it or a rotation.
pub fn check_algorithm(
    set: &AlgSet,
    case: &AlgCase,
    algorithm: &str,
) -> Result<(), CustomAlgError> {
    let candidate = notation::parse(algorithm).map_err(CustomAlgError::Notation)?;
    let Ok(setup) = setup(case) else {
        return Err(CustomAlgError::WrongCase);
    };
    let solves = (0..4).any(|quarters| {
        let mut turns = setup.clone();
        turns.push(auf(quarters));
        turns.extend_from_slice(&candidate);
        set.step.reached(&turns)
    });
    if solves {
        Ok(())
    } else {
        Err(CustomAlgError::WrongCase)
    }
}

/// A random turn on a face other than the last one's and its opposite.
fn random_opening_turn(previous: Option<Face>, random: &mut dyn RandomSource) -> Turn {
    const FACES: [Face; 6] = [Face::U, Face::R, Face::F, Face::D, Face::L, Face::B];
    let axis = |face: Face| face as usize % 3;
    loop {
        let face = FACES[bounded(random, FACES.len())];
        if previous.is_none_or(|previous| axis(previous) != axis(face)) {
            return Turn::new(face, 1 + bounded(random, 3) as u8);
        }
    }
}

/// Generates a scramble that leaves the cube in `case`, with a random turn of
/// the last layer on either side of it. The solver is fast enough here that
/// there is no need to prepare scrambles ahead.
pub fn generate_scramble(
    set: &AlgSet,
    case: &AlgCase,
    random: &mut dyn RandomSource,
) -> Result<String> {
    let mut target = set.step.opening(random)?;
    target.extend(setup(case)?);
    target.push(random_auf(random));

    let mut opening: Vec<Turn> = Vec::with_capacity(OPENING_TURNS);
    for _ in 0..OPENING_TURNS {
        let previous = opening.last().map(|turn: &Turn| turn.face);
        opening.push(random_opening_turn(previous, random));
    }

    // Solving what remains once the opening is undone gives the turns that
    // lead from the opening to the target.
    let mut remainder = invert(&opening);
    remainder.extend_from_slice(&target);
    let solution = Search::new().solution(
        &facelets(&remainder),
        MAX_SOLUTION,
        SOLVER_PROBES,
        0,
        INVERSE_SOLUTION,
    );
    let Some(solution) = parse_outer(&solution) else {
        bail!("The solver failed on {}: {solution}", case.name);
    };

    let scramble = simplify(opening.into_iter().chain(solution));
    if facelets(&scramble) != facelets(&target) {
        bail!("The scramble for {} leads somewhere else", case.name);
    }
    Ok(to_text(&scramble))
}

#[cfg(test)]
mod tests {
    use super::super::{OLL, all_sets};
    use super::*;
    use std::collections::HashSet;
    use tnoodle::java::JavaRandom;

    /// Reads the outer turns of a generated scramble.
    fn turns(scramble: &str) -> Vec<Turn> {
        parse_outer(scramble).unwrap()
    }

    /// Which stickers of the last layer's pieces face up, going clockwise as
    /// seen from above: first those on top, then those on the sides.
    fn orientation(turns: &[Turn]) -> ([bool; 8], [bool; 12]) {
        let cube = facelets(turns);
        let cube = cube.as_bytes();
        let facing_up = |sticker: usize| cube[sticker] == b'U';
        (
            [0, 1, 2, 5, 8, 7, 6, 3].map(facing_up),
            [47, 46, 45, 11, 10, 9, 20, 19, 18, 38, 37, 36].map(facing_up),
        )
    }

    /// The orientation pattern turned to its smallest form, so that the same
    /// case at another angle compares equal.
    fn canonical_orientation(turns: &[Turn]) -> ([bool; 8], [bool; 12]) {
        let mut pattern = orientation(turns);
        let mut smallest = pattern;
        for _ in 0..3 {
            pattern.0.rotate_left(2);
            pattern.1.rotate_left(3);
            smallest = smallest.min(pattern);
        }
        smallest
    }

    /// The side stickers of a permutation case, in their smallest form over
    /// every angle, every turn of the last layer, and which side is called
    /// which: those say nothing about the case.
    fn canonical_permutation(setup: &[Turn]) -> String {
        const SIDES: &[u8; 4] = b"FLBR";
        let renamed = |sticker: u8, shift: usize| {
            let place = SIDES.iter().position(|&side| side == sticker).unwrap();
            char::from(SIDES[(place + shift) % 4])
        };
        let mut forms = Vec::new();
        for before in 0..4 {
            for after in 0..4 {
                let mut turns = vec![auf(before)];
                turns.extend_from_slice(setup);
                turns.push(auf(after));
                let cube = facelets(&turns);
                let cube = cube.as_bytes();
                for shift in 0..4 {
                    let stickers = [9, 10, 11, 18, 19, 20, 36, 37, 38, 45, 46, 47];
                    forms.push(
                        stickers
                            .iter()
                            .map(|&sticker| renamed(cube[sticker], shift))
                            .collect::<String>(),
                    );
                }
            }
        }
        forms.into_iter().min().unwrap()
    }

    #[test]
    fn every_default_algorithm_solves_its_case() {
        for set in all_sets() {
            for case in set.cases {
                assert_eq!(
                    check_algorithm(set, case, case.algorithm),
                    Ok(()),
                    "{} of {}",
                    case.name,
                    set.name
                );
            }
        }
    }

    #[test]
    fn every_pll_keeps_the_first_two_layers_and_the_orientation() {
        for case in PLL.cases {
            let cube = facelets(&setup(case).unwrap());
            let cube = cube.as_bytes();
            assert!(first_two_layers_solved(cube), "{}", case.name);
            assert!(
                cube[..9].iter().all(|&sticker| sticker == b'U'),
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn every_oll_keeps_the_first_two_layers() {
        for case in OLL.cases {
            let cube = facelets(&setup(case).unwrap());
            assert!(first_two_layers_solved(cube.as_bytes()), "{}", case.name);
        }
    }

    #[test]
    fn every_oll_case_is_distinct() {
        let solved = canonical_orientation(&[]);
        let mut seen = HashSet::from([solved]);
        for case in OLL.cases {
            let pattern = canonical_orientation(&setup(case).unwrap());
            assert!(seen.insert(pattern), "{} repeats a case", case.name);
        }
        assert_eq!(seen.len(), 58);
    }

    #[test]
    fn every_pll_case_is_distinct() {
        let mut seen = HashSet::from([canonical_permutation(&[])]);
        for case in PLL.cases {
            let pattern = canonical_permutation(&setup(case).unwrap());
            assert!(seen.insert(pattern), "{} repeats a case", case.name);
        }
        assert_eq!(seen.len(), 22);
    }

    #[test]
    fn oll_groups_match_their_shape() {
        let edges_oriented = |case: &AlgCase| {
            let (top, _) = orientation(&setup(case).unwrap());
            [1, 3, 5, 7].iter().filter(|&&edge| top[edge]).count()
        };
        let corners_oriented = |case: &AlgCase| {
            let (top, _) = orientation(&setup(case).unwrap());
            [0, 2, 4, 6].iter().filter(|&&corner| top[corner]).count()
        };
        for case in OLL.cases {
            match case.group {
                "All edges oriented" => assert_eq!(edges_oriented(case), 4, "{}", case.name),
                "Dots" => assert_eq!(edges_oriented(case), 0, "{}", case.name),
                "Corners oriented" => assert_eq!(corners_oriented(case), 4, "{}", case.name),
                "Lightning" | "L shapes" | "Knights" | "Fish" | "Awkward" | "W shapes"
                | "C shapes" | "P shapes" | "T shapes" | "Squares" | "I shapes" => {
                    assert_ne!(edges_oriented(case), 4, "{}", case.name);
                }
                group => panic!("Unknown group {group}"),
            }
        }
    }

    #[test]
    fn well_known_cases_have_their_shape() {
        let edges = |name: &str| {
            let (top, _) = orientation(&setup(OLL.case(name).unwrap()).unwrap());
            [1, 3, 5, 7].map(|edge| top[edge])
        };
        // Sune and antisune: a cross and one corner.
        for name in ["OLL 26", "OLL 27"] {
            assert_eq!(edges(name), [true; 4]);
            let (top, _) = orientation(&setup(OLL.case(name).unwrap()).unwrap());
            let corners = [0, 2, 4, 6].iter().filter(|&&corner| top[corner]);
            assert_eq!(corners.count(), 1);
        }
    }

    #[test]
    fn a_custom_algorithm_may_solve_the_case_differently() {
        let t = PLL.case("T").unwrap();
        let other = "R U R' U' R' F R2 U' R' U' R U R' F'";
        assert_eq!(check_algorithm(&PLL, t, other), Ok(()));
        // The same algorithm seen from another angle.
        assert_eq!(check_algorithm(&PLL, t, &format!("y {other}")), Ok(()));
        assert_eq!(check_algorithm(&PLL, t, &format!("U {other}")), Ok(()));
        assert_eq!(check_algorithm(&PLL, t, &format!("{other} U2")), Ok(()));
    }

    #[test]
    fn wide_moves_are_expanded_before_checking() {
        let case = OLL.case("OLL 24").unwrap();
        assert_eq!(check_algorithm(&OLL, case, "r U R' U' r' F R F'"), Ok(()));
    }

    #[test]
    fn an_algorithm_for_another_case_is_rejected() {
        let t = PLL.case("T").unwrap();
        let z = PLL.case("Z").unwrap();
        assert_eq!(
            check_algorithm(&PLL, t, z.algorithm),
            Err(CustomAlgError::WrongCase)
        );
    }

    #[test]
    fn a_typo_in_an_algorithm_is_rejected() {
        let t = PLL.case("T").unwrap();
        assert_eq!(
            check_algorithm(&PLL, t, "R U R' U R' F R2 U' R' U' R U R' F'"),
            Err(CustomAlgError::WrongCase)
        );
    }

    #[test]
    fn unreadable_algorithms_are_reported() {
        let t = PLL.case("T").unwrap();
        assert_eq!(
            check_algorithm(&PLL, t, "R U Q"),
            Err(CustomAlgError::Notation(AlgError::InvalidMove(
                "Q".to_owned()
            )))
        );
    }

    #[test]
    fn every_generated_scramble_produces_its_case() {
        let mut random = JavaRandom::new(11);
        for set in all_sets() {
            for case in set.cases {
                for _ in 0..8 {
                    let scramble = generate_scramble(set, case, &mut random).unwrap();
                    let turns = turns(&scramble);
                    match set.step {
                        // The pattern is the case's, at some angle.
                        Step::OrientLastLayer => {
                            let expected = canonical_orientation(&setup(case).unwrap());
                            assert_eq!(canonical_orientation(&turns), expected, "{}", case.name);
                            assert!(
                                first_two_layers_solved(facelets(&turns).as_bytes()),
                                "{}",
                                case.name
                            );
                        }
                        // The algorithm solves it, from some angle.
                        Step::PermuteLastLayer => {
                            let candidate = notation::parse(case.algorithm).unwrap();
                            let solved = (0..4).any(|quarters| {
                                let mut attempt = turns.clone();
                                attempt.push(auf(quarters));
                                attempt.extend_from_slice(&candidate);
                                set.step.reached(&attempt)
                            });
                            assert!(solved, "{}", case.name);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_case_gets_many_different_scrambles() {
        let mut random = JavaRandom::new(12);
        let case = PLL.case("H").unwrap();
        let scrambles: HashSet<String> = (0..100)
            .map(|_| generate_scramble(&PLL, case, &mut random).unwrap())
            .collect();
        assert!(scrambles.len() > 80, "only {}", scrambles.len());
    }

    #[test]
    fn scrambles_are_not_the_literal_inverse_of_the_algorithm() {
        let mut random = JavaRandom::new(13);
        let case = OLL.case("OLL 27").unwrap();
        let inverse = to_text(&setup(case).unwrap());
        for _ in 0..50 {
            let scramble = generate_scramble(&OLL, case, &mut random).unwrap();
            assert!(!scramble.contains(&inverse));
        }
    }

    #[test]
    fn scrambles_stay_a_reasonable_length() {
        let mut random = JavaRandom::new(14);
        for case in OLL.cases {
            let scramble = generate_scramble(&OLL, case, &mut random).unwrap();
            let length = scramble.split_whitespace().count();
            assert!(length <= MAX_SOLUTION as usize + OPENING_TURNS, "{length}");
        }
    }
}
