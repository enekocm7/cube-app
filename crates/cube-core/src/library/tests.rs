//! Checks the library's data as a whole.
//!
//! Every puzzle has a model, so every listed algorithm is run. A case is
//! defined by its default algorithm, though, so for hand-written and generated
//! sets "it solves its case" says little about that default: the tests that
//! carry the weight there are that it starts from the set's `before`, that the
//! cases differ, and how many there are.

use std::collections::{HashMap, HashSet};

use super::{case_origins, case_state, catalog, check, find_set, library_sets, recognition};
use crate::event::WcaEvent;
use crate::trainer::{OLL, PLL, Step, puzzles};

#[test]
fn every_puzzle_has_a_set() {
    let events: HashSet<WcaEvent> = catalog().iter().map(|puzzle| puzzle.event).collect();
    for event in [
        WcaEvent::Cube2x2,
        WcaEvent::Cube3x3,
        WcaEvent::Cube4x4,
        WcaEvent::Cube5x5,
        WcaEvent::Cube6x6,
        WcaEvent::Cube7x7,
        WcaEvent::Megaminx,
        WcaEvent::Pyraminx,
        WcaEvent::Skewb,
        WcaEvent::Square1,
        WcaEvent::Clock,
        WcaEvent::Fto,
    ] {
        assert!(events.contains(&event), "{event:?}");
    }
    for puzzle in catalog() {
        assert!(!puzzle.sets.is_empty(), "{:?}", puzzle.event);
    }
}

#[test]
fn set_ids_are_unique() {
    let mut seen = HashSet::new();
    for set in library_sets() {
        assert!(seen.insert(set.id), "{}", set.id);
    }
}

#[test]
fn a_set_has_cases_with_unique_names_in_unbroken_groups() {
    for set in library_sets() {
        assert!(!set.cases.is_empty(), "{}", set.id);
        let mut names = HashSet::new();
        let mut closed = HashSet::new();
        let mut group = None;
        for case in set.cases {
            assert!(names.insert(case.name), "{} {}", set.id, case.name);
            if group != Some(case.group) {
                if let Some(previous) = group {
                    closed.insert(previous);
                }
                assert!(!closed.contains(case.group), "{} {}", set.id, case.group);
                group = Some(case.group);
            }
        }
    }
}

#[test]
fn every_case_starts_from_what_its_set_has_solved() {
    for set in library_sets() {
        for case in set.cases {
            let start = case_state(set, case);
            assert!(start.is_some(), "{} {}", set.id, case.name);
        }
    }
}

#[test]
fn every_listed_algorithm_solves_its_case() {
    for set in library_sets() {
        for case in set.cases {
            assert_eq!(case.algorithms[0], case.algorithm, "{}", case.name);
            for algorithm in case.algorithms {
                let solved = check(set, case, algorithm);
                assert_eq!(solved, Ok(()), "{} {}: {algorithm}", set.id, case.name);
            }
        }
    }
}

#[test]
fn the_cases_of_a_set_are_not_solved_already() {
    for set in library_sets() {
        let model = set.model();
        for case in set.cases {
            let start = case_state(set, case).unwrap();
            assert_ne!(start, model.solved(), "{} {}", set.id, case.name);
        }
    }
}

#[test]
fn the_named_sets_have_the_cases_they_are_known_for() {
    let counts: HashMap<&str, usize> = HashMap::from([
        ("pll", 21),
        ("oll", 57),
        ("f2l", 41),
        ("coll", 40),
        ("zbll-u", 72),
        ("zbll-h", 40),
        ("cmll", 42),
        ("2x2-cll", 40),
        ("2x2-ortega-oll", 7),
        ("2look-oll-edges", 3),
        ("2look-oll-corners", 7),
        ("2look-pll-corners", 2),
        ("2look-pll-edges", 4),
        // Every position of four edges, less the solved one.
        ("pyra-l4e", 37),
        // Cube shape is 170 shapes, and each has two parities.
        ("sq1-cubeshape", 168),
        ("sq1-csp", 340),
    ]);
    for (id, count) in counts {
        assert_eq!(find_set(id).unwrap().cases.len(), count, "{id}");
    }
}

#[test]
fn the_last_layer_sets_have_no_case_twice() {
    // Two cases are the same when one solves the other.
    for id in ["pll", "oll", "2x2-cll", "cmll", "pyra-ll", "mega-cp"] {
        let set = find_set(id).unwrap();
        for (index, case) in set.cases.iter().enumerate() {
            for other in &set.cases[index + 1..] {
                let same = check(set, case, other.algorithm).is_ok();
                assert!(!same, "{id}: {} and {}", case.name, other.name);
            }
        }
    }
}

#[test]
fn the_trainer_drills_the_library_s_own_sets() {
    assert!(std::ptr::eq(*PLL, find_set("pll").unwrap()));
    assert!(std::ptr::eq(*OLL, find_set("oll").unwrap()));
    for puzzle in puzzles() {
        for set in &puzzle.sets {
            assert_ne!(set.step, Step::Reference, "{}", set.id);
            assert!(std::ptr::eq(*set, find_set(set.id).unwrap()), "{}", set.id);
        }
    }
}

#[test]
fn a_case_follows_where_its_stickers_began() {
    let set = find_set("pll").unwrap();
    let case = set.case("T").unwrap();
    let origins = case_origins(set, case).unwrap();
    let mut sorted = origins.clone();
    sorted.sort_unstable();
    assert!(sorted.iter().copied().eq(0..origins.len() as u16));
    assert!(
        origins
            .iter()
            .enumerate()
            .any(|(at, &from)| at != usize::from(from))
    );
}

#[test]
fn recognition_counts_what_faces_up() {
    let set = find_set("oll").unwrap();
    let solved_top = recognition(set, set.case("OLL 27").unwrap()).unwrap();
    assert_eq!(solved_top.top_stickers, 9);
    assert!(solved_top.facing_up < 9);
    let permuted = find_set("pll").unwrap();
    let t = recognition(permuted, permuted.case("T").unwrap()).unwrap();
    assert_eq!(t.facing_up, 9);
    assert!(t.moved > 0);
    let clock = find_set("clock-flips").unwrap();
    assert_eq!(recognition(clock, &clock.cases[0]), None);
}

#[test]
fn a_search_looks_through_every_set() {
    let found = super::search_cases("sune");
    assert!(found.iter().any(|(set, _)| set.id == "2x2-cll"));
    assert!(found.iter().any(|(set, _)| set.id == "2look-oll-corners"));
    let pll = super::search_cases("pll 3x3x3");
    assert!(pll.iter().all(|(set, _)| set.event == WcaEvent::Cube3x3));
    assert!(super::search_cases("no such case").is_empty());
}

#[test]
fn a_puzzle_lists_each_method_once_with_all_its_sets() {
    for puzzle in catalog() {
        let methods = puzzle.methods();
        let names: HashSet<&str> = methods.iter().map(|(name, _)| *name).collect();
        assert_eq!(names.len(), methods.len(), "{:?}", puzzle.event);
        let sets: usize = methods.iter().map(|(_, sets)| sets.len()).sum();
        assert_eq!(sets, puzzle.sets.len(), "{:?}", puzzle.event);
    }
    let cube = catalog()
        .iter()
        .find(|puzzle| puzzle.event == WcaEvent::Cube3x3);
    let methods = cube.unwrap().methods();
    assert_eq!(methods[0].0, "CFOP");
    assert!(methods[0].1.iter().any(|set| set.id == "pll"));
}
