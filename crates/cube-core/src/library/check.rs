//! What a set's cases mean on the puzzle: where each one starts, and whether
//! an algorithm solves it.

use super::model::{Model, Op, State};
use crate::trainer::{AlgCase, AlgSet, CustomAlgError};

/// How many times a move of `turn` can usefully be repeated.
fn turn_limit(model: &Model, op: Op) -> u8 {
    match (model, op) {
        (Model::Stickers(stickers), Op::Turn { base, .. }) => stickers.order(base),
        (_, Op::Layers { top, bottom }) => {
            12 / gcd(gcd(top.unsigned_abs(), bottom.unsigned_abs()), 12)
        }
        _ => 1,
    }
}

const fn gcd(a: u8, b: u8) -> u8 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Returns every combination of the turns a set allows around an algorithm,
/// including none at all.
pub fn free_turns(set: &AlgSet) -> Vec<Vec<Op>> {
    let model = set.model();
    let mut combinations: Vec<Vec<Op>> = vec![Vec::new()];
    for word in set.turn_text.split_whitespace() {
        let Ok(ops) = model.parse(word) else {
            continue;
        };
        let Some(&op) = ops.first() else {
            continue;
        };
        let mut next = Vec::new();
        for combination in &combinations {
            for repeats in 0..turn_limit(model, op) {
                let mut longer = combination.clone();
                longer.extend(std::iter::repeat_n(op, repeats as usize));
                next.push(longer);
            }
        }
        combinations = next;
    }
    combinations
}

/// Returns the position an algorithm of `case` starts from, held so that what
/// the set needs solved is in place. An algorithm that rotates the puzzle
/// would otherwise leave it held some other way.
pub fn case_state(set: &AlgSet, case: &AlgCase) -> Option<State> {
    let model = set.model();
    let ops = model.parse(case.algorithm).ok()?;
    let mut state = model.solved();
    if !model.apply(&mut state, &model.inverse(&ops)) {
        return None;
    }
    let held = model.holds(&state);
    held.into_iter()
        .find(|hold| model.matches(hold, set.before()))
}

/// Returns whether `state` finished what the set sets out to do, however the
/// puzzle ended up held and with any of the allowed turns still to do.
fn finished(set: &AlgSet, state: &[u16], turns: &[Vec<Op>]) -> bool {
    let model = set.model();
    model.holds(state).iter().any(|hold| {
        turns.iter().any(|turn| {
            let mut last = hold.clone();
            model.apply(&mut last, turn) && model.matches(&last, set.after())
        })
    })
}

/// Checks that `algorithm` solves `case`, from any of the angles the set
/// allows: an algorithm may start with a turn of the top or a rotation.
pub fn check(set: &AlgSet, case: &AlgCase, algorithm: &str) -> Result<(), CustomAlgError> {
    let model = set.model();
    let ops = model.parse(algorithm).map_err(CustomAlgError::Notation)?;
    let start = case_state(set, case).ok_or(CustomAlgError::WrongCase)?;
    let turns = free_turns(set);
    let solves = turns.iter().any(|turn| {
        let mut state = start.clone();
        model.apply(&mut state, turn)
            && model.apply(&mut state, &ops)
            && finished(set, &state, &turns)
    });
    if solves {
        Ok(())
    } else {
        Err(CustomAlgError::WrongCase)
    }
}
