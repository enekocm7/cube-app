//! Scrambles for the cases of a 2x2x2, which `min2phase` can't solve.
//!
//! A 2x2x2 has few enough positions to search its own model: every position
//! within a few turns of solved is remembered once, and a case is reached by
//! walking from it until one of those is met. Only `R`, `U` and `F` are used,
//! which keeps one corner fixed so that a position has one spelling.

use std::collections::HashMap;
use std::sync::LazyLock;

use anyhow::{Result, anyhow, bail};
use tnoodle::java::RandomSource;

use super::deck::bounded;
use super::{AlgCase, AlgSet};
use crate::library::free_turns;
use crate::library::model::{Model, Op, State, StickerModel};

/// How far from solved the remembered positions are. A walk of five more
/// turns reaches every position of the puzzle, which is 11 from solved at most.
const REMEMBERED: usize = 6;
const WALK: usize = 5;

/// Random turns a scramble ends with, so a case has many scrambles and not
/// the one the search finds first.
const CLOSING_TURNS: usize = 3;

/// The turns the search uses: each of `R`, `U` and `F` in all three amounts.
struct Search {
    ops: Vec<Op>,
    /// The face of each of `ops`.
    faces: Vec<usize>,
    texts: Vec<String>,
    /// The places no turn of the search moves, which hold the fixed corner.
    fixed: Vec<usize>,
    /// The positions near solved, with the turn that led to each.
    near: HashMap<State, usize>,
}

impl Search {
    fn new(model: &Model, stickers: &StickerModel) -> Option<Self> {
        let mut ops = Vec::new();
        let mut faces = Vec::new();
        let mut texts = Vec::new();
        for (face, name) in ["U", "R", "F"].into_iter().enumerate() {
            let base = stickers.base(name)?;
            for power in 1..stickers.order(base) {
                ops.push(Op::Turn { base, power });
                faces.push(face);
                texts.push(stickers.text(base, power));
            }
        }
        let places = stickers.stickers.len();
        let fixed = (0..places)
            .filter(|&place| {
                ops.iter().all(|op| match *op {
                    Op::Turn { base, .. } => stickers.source(base)[place] as usize == place,
                    _ => false,
                })
            })
            .collect();

        let mut search = Self {
            ops,
            faces,
            texts,
            fixed,
            near: HashMap::new(),
        };
        let solved = model.solved();
        let mut layer = vec![solved.clone()];
        search.near.insert(solved, usize::MAX);
        for _ in 0..REMEMBERED {
            let mut next = Vec::new();
            for state in &layer {
                for (index, op) in search.ops.iter().enumerate() {
                    let mut moved = state.clone();
                    model.apply(&mut moved, &[*op]);
                    if !search.near.contains_key(&moved) {
                        search.near.insert(moved.clone(), index);
                        next.push(moved);
                    }
                }
            }
            layer = next;
        }
        Some(search)
    }

    /// Returns the turn that undoes the turn at `index`.
    fn undo(index: usize) -> usize {
        // The three amounts of a face are next to each other: 1, 2, 3 quarters.
        let first = index / 3 * 3;
        first + 2 - (index - first)
    }

    /// Walks from `state` until a remembered position turns up, and returns
    /// the turns from solved to `state`.
    fn walk(&self, model: &Model, state: &State) -> Option<Vec<usize>> {
        let mut walked = Vec::new();
        self.walk_from(model, state, None, &mut walked)
            .map(|reached| self.join(model, reached, &walked))
    }

    fn walk_from(
        &self,
        model: &Model,
        state: &State,
        last_face: Option<usize>,
        walked: &mut Vec<usize>,
    ) -> Option<State> {
        if self.near.contains_key(state) {
            return Some(state.clone());
        }
        if walked.len() == WALK {
            return None;
        }
        for (index, op) in self.ops.iter().enumerate() {
            if last_face == Some(self.faces[index]) {
                continue;
            }
            let mut moved = state.clone();
            model.apply(&mut moved, &[*op]);
            walked.push(index);
            if let Some(reached) = self.walk_from(model, &moved, Some(self.faces[index]), walked) {
                return Some(reached);
            }
            walked.pop();
        }
        None
    }

    /// Returns the turns from solved to the position that `walked` started
    /// from: the way to `reached`, then the walk undone.
    fn join(&self, model: &Model, reached: State, walked: &[usize]) -> Vec<usize> {
        let mut path = Vec::new();
        let mut state = reached;
        loop {
            let index = self.near[&state];
            if index == usize::MAX {
                break;
            }
            path.push(index);
            let mut back = state;
            model.apply(&mut back, &[self.ops[Self::undo(index)]]);
            state = back;
        }
        path.reverse();
        path.extend(walked.iter().rev().map(|&index| Self::undo(index)));
        path
    }
}

/// Searches the 2x2x2 the first time.
fn search() -> Option<&'static Search> {
    static SEARCH: LazyLock<Option<Search>> = LazyLock::new(|| {
        let model = Model::of(crate::event::WcaEvent::Cube2x2);
        let Model::Stickers(stickers) = model else {
            return None;
        };
        Search::new(model, stickers)
    });
    SEARCH.as_ref()
}

/// Scrambles into a case of a puzzle with no solver here by undoing its
/// algorithm, between a random free turn on either side. The scramble is
/// recognisable as the algorithm backwards, but it is always a valid case.
fn inverse_scramble(set: &AlgSet, algorithm: &[Op], random: &mut dyn RandomSource) -> String {
    let model = set.model();
    let free = free_turns(set);
    let mut ops = free[bounded(random, free.len())].clone();
    ops.extend(model.inverse(algorithm));
    ops.extend(free[bounded(random, free.len())].iter().copied());
    model.write(&model.tidy(&ops))
}

/// Generates a scramble that leaves a 2x2x2 in `case`, or any other puzzle
/// the library has, by undoing the algorithm.
pub(super) fn generate(
    set: &AlgSet,
    case: &AlgCase,
    random: &mut dyn RandomSource,
) -> Result<String> {
    let model = set.model();
    let algorithm = model
        .parse(case.algorithm)
        .map_err(|error| anyhow!("Invalid algorithm for {}: {error:?}", case.name))?;
    let Some(search) = search().filter(|_| set.event == crate::event::WcaEvent::Cube2x2) else {
        return Ok(inverse_scramble(set, &algorithm, random));
    };

    // Set the case up between a free turn on either side of it.
    let free = free_turns(set);
    let mut target = model.solved();
    model.apply(&mut target, &free[bounded(random, free.len())]);
    model.apply(&mut target, &model.inverse(&algorithm));
    model.apply(&mut target, &free[bounded(random, free.len())]);

    // The search keeps one corner in place, so hold the puzzle that way.
    let solved = model.solved();
    let held = model.holds(&target).into_iter().find(|hold| {
        search
            .fixed
            .iter()
            .all(|&place| hold[place] == solved[place])
    });
    let Some(held) = held else {
        bail!("{} can't be held with its fixed corner in place", case.name);
    };

    // A few random turns to end on, undone before searching.
    let mut closing: Vec<usize> = Vec::new();
    for _ in 0..CLOSING_TURNS {
        let face = closing.last().map(|&index| search.faces[index]);
        let index = loop {
            let index = bounded(random, search.ops.len());
            if face != Some(search.faces[index]) {
                break index;
            }
        };
        closing.push(index);
    }
    let mut remainder = held;
    for &index in closing.iter().rev() {
        model.apply(&mut remainder, &[search.ops[Search::undo(index)]]);
    }
    let Some(path) = search.walk(model, &remainder) else {
        bail!("The search failed on {}", case.name);
    };

    let turns = path.iter().chain(&closing);
    let text: Vec<&str> = turns.map(|&index| search.texts[index].as_str()).collect();
    Ok(text.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{find_set, solves_from};
    use tnoodle::java::JavaRandom;

    #[test]
    fn a_case_is_reached_by_its_scramble() {
        let set = find_set("2x2-cll").unwrap();
        let mut random = JavaRandom::new(31);
        for case in set.cases {
            let scramble = generate(set, case, &mut random).unwrap();
            let model = set.model();
            let mut state = model.solved();
            model.apply(&mut state, &model.parse(&scramble).unwrap());
            let algorithm = model.parse(case.algorithm).unwrap();
            assert!(solves_from(set, &state, &algorithm), "{}", case.name);
        }
    }

    #[test]
    fn a_puzzle_without_a_search_undoes_the_algorithm() {
        let mut random = JavaRandom::new(32);
        let set = find_set("sq1-cubeshape").unwrap();
        let scramble = generate(set, &set.cases[0], &mut random).unwrap();
        assert_ne!(scramble, "");
    }

    #[test]
    fn an_undone_algorithm_has_no_rotations_or_repeated_turns() {
        use crate::event::WcaEvent;
        use crate::trainer::all_sets;
        let mut random = JavaRandom::new(33);
        let cubes = [
            WcaEvent::Cube4x4,
            WcaEvent::Cube5x5,
            WcaEvent::Cube6x6,
            WcaEvent::Cube7x7,
        ];
        for set in all_sets().filter(|set| cubes.contains(&set.event)) {
            let model = set.model();
            for case in set.cases {
                let scramble = generate(set, case, &mut random).unwrap();
                let words: Vec<&str> = scramble.split_whitespace().collect();
                assert!(
                    words.iter().all(|word| !"xyz".contains(&word[..1])),
                    "{scramble}"
                );
                let ops = model.parse(&scramble).unwrap();
                for pair in ops.windows(2) {
                    let same = matches!(pair, [Op::Turn { base: a, .. }, Op::Turn { base: b, .. }] if a == b);
                    assert!(!same, "{scramble}");
                }
            }
        }
    }
}
