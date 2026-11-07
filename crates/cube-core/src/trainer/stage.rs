//! Stages of a CFOP solve as something to drill: instead of one algorithm
//! case, a random state with everything before the stage solved, such as the
//! last layer of a cube with its first two layers done.
//!
//! The stages are one set, [`STAGES`], so that choosing them, drilling them
//! and keeping their attempts work like any other set's cases.

use std::sync::{LazyLock, OnceLock};

use anyhow::{Result, bail};
use tnoodle::java::RandomSource;
use tnoodle::min2phase::tools::{PieceSpec, random_cross_solved, random_last_layer, random_state};

use super::cube::solve_facelets;
use super::notation::to_text;
use super::{AlgCase, AlgSet, Step};
use crate::event::WcaEvent;

/// A stage's case, which has no algorithm to define it.
const fn stage(name: &'static str, group: &'static str, recognition: &'static str) -> AlgCase {
    AlgCase {
        name,
        group,
        recognition,
        algorithm: "",
        algorithms: &[],
    }
}

const CASES: [AlgCase; 6] = [
    stage(
        "Last layer",
        "Stages",
        "Cross and F2L solved, the last layer random.",
    ),
    stage("F2L", "Stages", "Cross solved, everything else random."),
    stage(
        "Front right slot",
        "Single slot",
        "Cross and the other three slots solved.",
    ),
    stage(
        "Front left slot",
        "Single slot",
        "Cross and the other three slots solved.",
    ),
    stage(
        "Back left slot",
        "Single slot",
        "Cross and the other three slots solved.",
    ),
    stage(
        "Back right slot",
        "Single slot",
        "Cross and the other three slots solved.",
    ),
];

/// The stages of a 3x3x3 solve, which the trainer drills like a set.
pub static STAGES: LazyLock<&'static AlgSet> = LazyLock::new(|| {
    Box::leak(Box::new(AlgSet {
        id: "stages",
        name: "Stages",
        method: "CFOP",
        event: WcaEvent::Cube3x3,
        step: Step::Stage,
        before_text: "all",
        after_text: "all",
        turn_text: "",
        show_text: "all",
        solid_view: false,
        source: "",
        cases: &CASES,
        regions: OnceLock::new(),
    }))
});

/// The corner and the edge of each slot, by the solver's numbering, in the
/// order of the slot cases. The cross is edges 4 to 7.
const SLOTS: [(usize, usize); 4] = [(4, 8), (5, 9), (6, 10), (7, 11)];

/// A piece spec that leaves the pieces at `free` unknown and the rest solved.
/// Pieces 0 to 3 are those of the last layer.
fn solved_but(count: usize, free: &[usize], orientation: bool) -> PieceSpec {
    let spec = (0..count)
        .map(|piece| {
            if free.contains(&piece) {
                -1
            } else if orientation {
                0
            } else {
                piece as i8
            }
        })
        .collect();
    PieceSpec::Partial(spec)
}

/// Returns a random cube in the stage that `case` names.
fn state(case: &AlgCase, random: &mut dyn RandomSource) -> Option<String> {
    if case.name == "Last layer" {
        return Some(random_last_layer(random));
    }
    if case.name == "F2L" {
        return Some(random_cross_solved(random));
    }
    let slot = CASES[2..]
        .iter()
        .position(|other| other.name == case.name)?;
    let (corner, edge) = SLOTS[slot];
    let corners: Vec<usize> = (0..4).chain([corner]).collect();
    let edges: Vec<usize> = (0..4).chain([edge]).collect();
    Some(random_state(
        solved_but(8, &corners, false),
        solved_but(8, &corners, true),
        solved_but(12, &edges, false),
        solved_but(12, &edges, true),
        random,
    ))
}

/// Generates a scramble into the stage that `case` names.
pub(super) fn generate(case: &AlgCase, random: &mut dyn RandomSource) -> Result<String> {
    let Some(facelets) = state(case, random) else {
        bail!("{} is not a stage", case.name);
    };
    Ok(to_text(&solve_facelets(&facelets)?))
}

#[cfg(test)]
mod tests {
    use super::super::cube::facelets;
    use super::super::notation::parse_outer;
    use super::*;
    use tnoodle::java::JavaRandom;

    /// Where the stickers of the four cross edges sit, with the colors they
    /// show when solved.
    const CROSS: [(usize, u8); 8] = [
        (28, b'D'),
        (25, b'F'),
        (30, b'D'),
        (43, b'L'),
        (32, b'D'),
        (16, b'R'),
        (34, b'D'),
        (52, b'B'),
    ];

    /// The stickers of each slot's corner and edge, with their solved colors,
    /// in the order of the slot cases.
    const SLOT_STICKERS: [[(usize, u8); 5]; 4] = [
        [(29, b'D'), (26, b'F'), (15, b'R'), (23, b'F'), (12, b'R')],
        [(27, b'D'), (24, b'F'), (44, b'L'), (21, b'F'), (41, b'L')],
        [(33, b'D'), (42, b'L'), (53, b'B'), (39, b'L'), (50, b'B')],
        [(35, b'D'), (17, b'R'), (51, b'B'), (14, b'R'), (48, b'B')],
    ];

    fn solved_at(cube: &[u8], stickers: &[(usize, u8)]) -> bool {
        stickers.iter().all(|&(place, color)| cube[place] == color)
    }

    fn stage_cube(name: &str, seed: i64) -> String {
        let mut random = JavaRandom::new(seed);
        let case = STAGES.case(name).unwrap();
        let scramble = generate(case, &mut random).unwrap();
        facelets(&parse_outer(&scramble).unwrap())
    }

    #[test]
    fn the_last_layer_stage_leaves_the_first_two_layers_solved() {
        for seed in 0..20 {
            let cube = stage_cube("Last layer", seed);
            let cube = cube.as_bytes();
            assert!(solved_at(cube, &CROSS));
            for slot in &SLOT_STICKERS {
                assert!(solved_at(cube, slot));
            }
            assert!(cube[27..36].iter().all(|&sticker| sticker == b'D'));
        }
    }

    #[test]
    fn the_f2l_stage_leaves_only_the_cross_solved() {
        let mut unsolved_slots = 0;
        for seed in 0..20 {
            let cube = stage_cube("F2L", seed);
            let cube = cube.as_bytes();
            assert!(solved_at(cube, &CROSS));
            unsolved_slots += SLOT_STICKERS
                .iter()
                .filter(|slot| !solved_at(cube, &slot[..]))
                .count();
        }
        assert!(unsolved_slots > 20);
    }

    #[test]
    fn a_slot_stage_leaves_the_other_slots_solved() {
        for (slot, name) in [
            "Front right slot",
            "Front left slot",
            "Back left slot",
            "Back right slot",
        ]
        .into_iter()
        .enumerate()
        {
            let mut unsolved = 0;
            for seed in 0..20 {
                let cube = stage_cube(name, seed);
                let cube = cube.as_bytes();
                assert!(solved_at(cube, &CROSS), "{name}");
                for (other, stickers) in SLOT_STICKERS.iter().enumerate() {
                    if other != slot {
                        assert!(solved_at(cube, stickers), "{name} broke slot {other}");
                    }
                }
                unsolved += usize::from(!solved_at(cube, &SLOT_STICKERS[slot]));
            }
            assert!(unsolved > 10, "{name} was mostly solved already");
        }
    }

    #[test]
    fn stages_come_in_many_different_states() {
        let scrambles: std::collections::HashSet<String> =
            (0..20).map(|seed| stage_cube("Last layer", seed)).collect();
        assert!(scrambles.len() > 15);
    }

    #[test]
    fn every_stage_is_a_case_of_the_set() {
        assert_eq!(STAGES.cases.len(), CASES.len());
        assert!(STAGES.is_stage());
        for case in STAGES.cases {
            assert!(
                state(case, &mut JavaRandom::new(1)).is_some(),
                "{}",
                case.name
            );
        }
    }
}
