//! Finds the library's Square-1 sets and prints them in the format of the data
//! files. Shapes are few, so cube shape and CSP search them outright; the
//! permutation sets join short sequences that reach the same shape.
//!
//! ```sh
//! cargo run --release -p cube-core --example library_square1
//! ```

#![allow(
    clippy::type_complexity,
    clippy::too_many_lines,
    clippy::assigning_clones,
    reason = "a tool run by hand to write the data files"
)]

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use cube_core::WcaEvent;
use cube_core::library::model::{Model, Op, State, square1_can_slash, square1_corner};

/// Paths kept for each shape.
const KEEP: usize = 3;
/// How many cases of a permutation set to keep, the shortest first.
const MOST_CASES: usize = 60;
/// How many slashes each half of a permutation search goes.
const HALF_DEPTH: usize = 4;
/// Sequences kept for one shape in a permutation search.
const GROUP_LIMIT: usize = 400;

type Path = Vec<(i8, i8)>;

fn turned(model: &Model, state: &State, top: i8, bottom: i8) -> State {
    let mut next = state.clone();
    model.apply(&mut next, &[Op::Layers { top, bottom }]);
    next
}

/// Every position one `(top,bottom)/` away.
fn successors(model: &Model, state: &State) -> Vec<(i8, i8, State)> {
    let mut found = Vec::new();
    for top in -5..=6 {
        for bottom in -5..=6 {
            let mut next = turned(model, state, top, bottom);
            if square1_can_slash(&next) {
                model.apply(&mut next, &[Op::Slash]);
                found.push((top, bottom, next));
            }
        }
    }
    found
}

/// A layer's corners and edges, from its best angle.
fn layer_shape(slots: &[u16]) -> Vec<bool> {
    let shape: Vec<bool> = slots.iter().map(|&piece| square1_corner(piece)).collect();
    (0..12)
        .map(|shift| {
            let mut turned = shape.clone();
            turned.rotate_left(shift);
            turned
        })
        .max()
        .unwrap_or_default()
}

/// A shape, the same from any angle of either layer.
fn shape_key(state: &[u16]) -> (Vec<bool>, Vec<bool>) {
    (layer_shape(&state[..12]), layer_shape(&state[12..24]))
}

/// A shape exactly as it stands, which `/` depends on.
fn exact_shape(state: &[u16]) -> Vec<bool> {
    let slots = state[..24].iter();
    slots.map(|&piece| square1_corner(piece)).collect()
}

/// Reports an odd permutation, the same way [`Model::matches`] reads `even`.
fn is_even(model: &Model, state: &[u16]) -> bool {
    let region = model.region("even").expect("even");
    model.matches(state, &region)
}

/// The moves that undo `path`, starting from where it ends.
fn undo_text(path: &Path) -> String {
    let mut text = String::new();
    for &(top, bottom) in path.iter().rev() {
        let _ = write!(text, "/ ({},{}) ", -top, -bottom);
    }
    text.trim().replace("(0,0) ", "").replace(" (0,0)", "")
}

/// Walks the shapes breadth first, keeping a few paths to each.
fn shape_paths<K: std::hash::Hash + Eq + Clone>(
    model: &Model,
    key: impl Fn(&[u16]) -> K,
) -> Vec<(K, Vec<Path>)> {
    let start = model.solved();
    let mut found: HashMap<K, Vec<Path>> = HashMap::new();
    let mut order: Vec<K> = Vec::new();
    let mut seen: HashSet<State> = HashSet::new();
    let mut frontier: Vec<(State, Path)> = vec![(start, Vec::new())];
    while !frontier.is_empty() {
        let mut next = Vec::new();
        for (state, path) in &frontier {
            for (top, bottom, moved) in successors(model, state) {
                let mut longer = path.clone();
                longer.push((top, bottom));
                let id = key(&moved);
                let known = found.get(&id).map_or(0, Vec::len);
                if known >= KEEP {
                    continue;
                }
                if known == 0 {
                    order.push(id.clone());
                }
                found.entry(id).or_default().push(longer.clone());
                if seen.insert(moved.clone()) {
                    next.push((moved, longer));
                }
            }
        }
        frontier = next;
    }
    order
        .into_iter()
        .map(|id| {
            let paths = found.remove(&id).unwrap_or_default();
            (id, paths)
        })
        .collect()
}

fn header(block: &mut String, id: &str, name: &str, before: &str, after: &str) {
    let _ = writeln!(block, "\nset {id}\nname {name}\nmethod Vandenbergh");
    let _ = writeln!(block, "before {before}\nafter {after}\nturn (3,0) (0,3)");
    let _ = writeln!(block, "show all\nview top\nsource generated");
}

/// Counts a layer's corners, which is how shapes are usually grouped.
fn corners(layer: &[bool]) -> usize {
    layer.iter().filter(|corner| **corner).count() / 2
}

fn shape_block(block: &mut String, shapes: Vec<((Vec<bool>, Vec<bool>), Vec<Path>, &str)>) {
    let mut groups: Vec<(String, Vec<(String, Vec<Path>)>)> = Vec::new();
    for ((top, bottom), paths, tag) in shapes {
        let group = format!("{}-{}", corners(&top), corners(&bottom));
        let at = groups.iter().position(|known| known.0 == group);
        let at = at.unwrap_or_else(|| {
            groups.push((group, Vec::new()));
            groups.len() - 1
        });
        let number = groups[at].1.len() + 1;
        let name = format!("Shape {} {number}{tag}", groups[at].0);
        groups[at].1.push((name, paths));
    }
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    for (group, cases) in groups {
        let _ = writeln!(block, "group {group}");
        for (name, paths) in cases {
            let texts: Vec<String> = paths.iter().map(undo_text).collect();
            let _ = writeln!(block, "case {name} |  | {}", texts.join(" | "));
        }
    }
}

/// Finds cube-shape positions, as what is out of place and the path to them.
fn cube_shape_cases(model: &Model) -> Vec<(State, String)> {
    let mut walked: Vec<(State, Path)> = vec![(model.solved(), Vec::new())];
    let mut seen: HashSet<State> = HashSet::from([model.solved()]);
    let mut next_from = 0;
    for _ in 0..HALF_DEPTH {
        let end = walked.len();
        for index in next_from..end {
            let (state, path) = walked[index].clone();
            for (top, bottom, moved) in successors(model, &state) {
                if seen.insert(moved.clone()) {
                    let mut longer = path.clone();
                    longer.push((top, bottom));
                    walked.push((moved, longer));
                }
            }
        }
        next_from = end;
    }
    eprintln!("square-1: {} positions", walked.len());
    let mut by_shape: HashMap<Vec<bool>, Vec<usize>> = HashMap::new();
    for (index, (state, _)) in walked.iter().enumerate() {
        let group = by_shape.entry(exact_shape(state)).or_default();
        if group.len() < GROUP_LIMIT {
            group.push(index);
        }
    }
    let mut found: HashMap<State, String> = HashMap::new();
    for group in by_shape.values() {
        for &first in group {
            for &second in group {
                if first == second {
                    continue;
                }
                let (from, path) = (&walked[first].0, &walked[first].1);
                let back = &walked[second].1;
                // There and back by another way: the shape returns, the pieces don't.
                let mut text = String::new();
                for &(top, bottom) in path {
                    let _ = write!(text, "({top},{bottom})/ ");
                }
                text.push_str(&undo_text(back));
                let Ok(ops) = model.parse(&text) else {
                    continue;
                };
                let mut state = model.solved();
                if !model.apply(&mut state, &ops) {
                    continue;
                }
                let _ = from;
                let entry = found.entry(state);
                let entry = entry.or_insert_with(|| text.clone());
                if text.len() < entry.len() {
                    entry.clone_from(&text);
                }
            }
        }
    }
    found.into_iter().collect()
}

/// The form of a position the same at every quarter turn of either layer.
fn case_key(model: &Model, state: &State) -> State {
    let mut best: Option<State> = None;
    for first in 0..4 {
        for second in 0..4 {
            let start = turned(model, state, first * 3, second * 3);
            if best.as_ref().is_none_or(|known| start < *known) {
                best = Some(start);
            }
        }
    }
    best.unwrap_or_default()
}

fn permutation_block(block: &mut String, prefix: &str, mut cases: Vec<String>) {
    cases.sort_by_key(String::len);
    cases.truncate(MOST_CASES);
    let _ = writeln!(block, "group {prefix}");
    for (number, text) in cases.iter().enumerate() {
        let _ = writeln!(block, "case {prefix} {} |  | {text}", number + 1);
    }
}

fn main() {
    let model = Model::of(WcaEvent::Square1);
    let mut block = String::from("# The Square-1 sets. See library/catalog.rs for the format.\n");

    let shapes = shape_paths(model, shape_key);
    eprintln!("square-1: {} shapes", shapes.len());
    header(&mut block, "sq1-cubeshape", "Cube shape", "", "shape");
    let shapes = shapes.into_iter().map(|(key, paths)| (key, paths, ""));
    shape_block(&mut block, shapes.collect());

    let csp = shape_paths(model, |state| (shape_key(state), is_even(model, state)));
    eprintln!("square-1: {} shapes with parity", csp.len());
    header(&mut block, "sq1-csp", "CSP", "", "shape even");
    let csp = csp
        .into_iter()
        .map(|((key, even), paths)| (key, paths, if even { " even" } else { " odd" }));
    shape_block(&mut block, csp.collect());

    let solved = model.solved();
    let corner_slots = model.region("corners").expect("corners").places;
    let edge_slots = model.region("edges").expect("edges").places;
    let home = |state: &[u16], slots: &[bool]| {
        let places = state.iter().zip(&solved).zip(slots);
        places
            .filter(|(_, chosen)| **chosen)
            .all(|(pair, _)| pair.0 == pair.1)
    };
    let (mut cp, mut ep, mut parity) = (HashMap::new(), HashMap::new(), HashMap::new());
    for (state, text) in cube_shape_cases(model) {
        if state == solved {
            continue;
        }
        let key = case_key(model, &state);
        let keep = |map: &mut HashMap<State, String>| {
            let entry = map.entry(key.clone()).or_insert_with(|| text.clone());
            if text.len() < entry.len() {
                entry.clone_from(&text);
            }
        };
        if !is_even(model, &state) && home(&state, &corner_slots) {
            keep(&mut parity);
        } else if home(&state, &edge_slots) && state[24] == 0 {
            keep(&mut cp);
        } else if home(&state, &corner_slots) && state[24] == 0 {
            keep(&mut ep);
        }
    }
    eprintln!(
        "square-1: {} corner, {} edge, {} parity cases",
        cp.len(),
        ep.len(),
        parity.len()
    );
    header(
        &mut block,
        "sq1-cp",
        "Corner permutation",
        "shape edges",
        "all",
    );
    permutation_block(&mut block, "CP", cp.into_values().collect());
    header(
        &mut block,
        "sq1-ep",
        "Edge permutation",
        "shape corners",
        "all",
    );
    permutation_block(&mut block, "EP", ep.into_values().collect());
    header(&mut block, "sq1-parity", "Parity", "shape corners", "all");
    permutation_block(&mut block, "Parity", parity.into_values().collect());
    print!("{block}");
}
