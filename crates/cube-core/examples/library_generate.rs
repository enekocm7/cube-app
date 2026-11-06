//! Finds algorithms for the library's sets that have no published source we can
//! copy, and prints them in the format of the data files.
//!
//! ```sh
//! cargo run --release -p cube-core --example library_generate -- <spec file>
//! ```
//!
//! A search explores every sequence of a set's moves up to a depth, keeps those
//! that leave the `keep` region solved, and files each under the case it
//! solves. Two half-length searches are joined on what they do to that region,
//! which reaches twice the depth for the same work. The shortest and easiest
//! algorithms of each case are written out, so every one is original and the
//! library's tests still check them all.
//!
//! A spec file is one block per set:
//!
//! ```text
//! set 2x2-cll             id of the set
//! event Cube2x2
//! name CLL
//! method CLL
//! before all -layer:U     solved in every case
//! after all               what an algorithm must solve
//! turn U                  turns free on either side
//! show all
//! view top
//! moves R U F             the moves to search with
//! depth 6                 how deep each half goes
//! keep 4                  algorithms to keep per case
//! prefix CLL              cases are named by this and a number
//! ```

use std::collections::HashMap;
use std::fmt::Write as _;

use cube_core::WcaEvent;
use cube_core::library::model::{Model, Region, State, StickerModel};

/// The most sequences kept for one effect on what must stay solved.
const GROUP_LIMIT: usize = 3_000;

/// A set to search for.
#[derive(Default, Clone)]
struct Spec {
    id: String,
    event: String,
    name: String,
    method: String,
    before: String,
    after: String,
    turn: String,
    show: String,
    view: String,
    moves: String,
    depth: usize,
    keep: usize,
    prefix: String,
    /// The most moves an algorithm may have.
    longest: usize,
    /// Compares only the colors of what must stay solved, so its pieces may
    /// swap among themselves.
    loose: bool,
    /// How many sides of the bottom layer must show a bar; `None` for any.
    bars: Option<usize>,
    /// Groups the cases by the shape of the top's corners.
    shapes: bool,
}

fn event_named(name: &str) -> WcaEvent {
    WcaEvent::ALL
        .into_iter()
        .find(|event| format!("{event:?}") == name)
        .unwrap_or_else(|| panic!("No event {name}"))
}

fn read_specs(text: &str) -> Vec<Spec> {
    let mut specs: Vec<Spec> = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        let value = value.trim().to_owned();
        if key == "set" {
            specs.push(Spec {
                id: value,
                keep: 4,
                longest: 99,
                ..Spec::default()
            });
            continue;
        }
        let spec = specs.last_mut().expect("A spec starts with set");
        match key {
            "event" => spec.event = value,
            "name" => spec.name = value,
            "method" => spec.method = value,
            "before" => spec.before = value,
            "after" => spec.after = value,
            "turn" => spec.turn = value,
            "show" => spec.show = value,
            "view" => spec.view = value,
            "moves" => spec.moves = value,
            "depth" => spec.depth = value.parse().expect("depth"),
            "keep" => spec.keep = value.parse().expect("keep"),
            "longest" => spec.longest = value.parse().expect("longest"),
            "prefix" => spec.prefix = value,
            "loose" => spec.loose = true,
            "bars" => spec.bars = Some(value.parse().expect("bars")),
            "shapes" => spec.shapes = true,
            _ => panic!("Unknown key {key}"),
        }
    }
    specs
}

/// One move of the search, with how it reads.
#[derive(Clone)]
struct Step {
    source: Vec<u16>,
    text: String,
    /// Moves of one family, which never follow each other.
    family: u16,
    cost: u32,
}

/// Returns every power of the moves named in `spec`.
fn steps(model: &StickerModel, moves: &str) -> Vec<Step> {
    let mut steps = Vec::new();
    for name in moves.split_whitespace() {
        let base = model.base(name).unwrap_or_else(|| panic!("No move {name}"));
        let once = model.source(base).to_vec();
        let mut source = once.clone();
        for power in 1..model.order(base) {
            let quarter = u32::from(power.min(model.order(base) - power));
            let awkward = match name.chars().next() {
                Some('R' | 'U') => 0,
                Some('r' | 'F' | 'L' | 'M') => 1,
                _ => 2,
            };
            steps.push(Step {
                source: source.clone(),
                text: model.text(base, power),
                family: base,
                cost: 10 * quarter + 6 * awkward + 4,
            });
            source = once.iter().map(|&from| source[from as usize]).collect();
        }
    }
    steps
}

/// A sequence found by a search.
#[derive(Clone)]
struct Found {
    state: State,
    moves: Vec<u8>,
}

/// Walks every sequence up to `depth`, never turning one family twice in a row.
fn walk(solved: &State, steps: &[Step], depth: usize) -> Vec<Found> {
    let mut found = vec![Found {
        state: solved.clone(),
        moves: Vec::new(),
    }];
    let mut frontier = 0;
    for _ in 0..depth {
        let end = found.len();
        for index in frontier..end {
            let last = found[index].moves.last().map(|&step| steps[step as usize].family);
            for (id, step) in steps.iter().enumerate() {
                if last == Some(step.family) {
                    continue;
                }
                let current = &found[index];
                let state: State = step
                    .source
                    .iter()
                    .map(|&from| current.state[from as usize])
                    .collect();
                let mut moves = current.moves.clone();
                moves.push(id as u8);
                found.push(Found { state, moves });
            }
        }
        frontier = end;
    }
    found
}

/// What a state looks like inside a region.
fn masked(state: &[u16], region: &Region) -> Vec<u16> {
    let places = state.iter().zip(&region.places);
    places
        .filter(|(_, chosen)| **chosen)
        .map(|(&place, _)| place)
        .collect()
}

/// Returns the turns allowed around an algorithm, as sticker permutations.
fn free_turns(model: &StickerModel, text: &str) -> Vec<Vec<u16>> {
    let identity: Vec<u16> = (0..model.stickers.len() as u16).collect();
    let mut turns = vec![identity];
    for name in text.split_whitespace() {
        let base = model.base(name).unwrap_or_else(|| panic!("No move {name}"));
        let once = model.source(base);
        let mut next = Vec::new();
        for turn in &turns {
            let mut current = turn.clone();
            for _ in 0..model.order(base) {
                next.push(current.clone());
                current = once.iter().map(|&from| current[from as usize]).collect();
            }
        }
        turns = next;
    }
    turns
}

fn permuted(state: &[u16], turn: &[u16]) -> State {
    turn.iter().map(|&from| state[from as usize]).collect()
}

/// The form of a case that is the same from every angle: the smallest of its
/// positions over the turns on either side.
fn case_key(state: &[u16], turns: &[Vec<u16>], numbering: &[u16], colors: &[u16]) -> State {
    let mut best: Option<State> = None;
    for before in turns {
        // A turn before the case rotates what it does; a recoloring keeps the
        // comparison about the case and not about which side is in front.
        let mut start = permuted(numbering, before);
        start = state.iter().map(|&place| start[place as usize]).collect();
        for after in turns {
            let finished: State = permuted(&start, after)
                .iter()
                .map(|&place| colors[place as usize])
                .collect();
            if best.as_ref().is_none_or(|known| finished < *known) {
                best = Some(finished);
            }
        }
    }
    best.unwrap_or_default()
}

/// Names of the top's corner shapes, in the order they are listed.
const SHAPES: [&str; 8] = ["Solved", "Sune", "Antisune", "Pi", "U", "T", "L", "H"];

/// Counts the sides of the bottom layer whose stickers all match each other.
fn bars(model: &StickerModel, state: &[u16], solved: &[u16], down: &[bool]) -> usize {
    let faces = model.face_names.len();
    (0..faces)
        .filter(|&face| model.face_names[face] != "D")
        .filter(|&face| {
            let on_face = model.stickers.iter().enumerate().filter(|(place, sticker)| {
                usize::from(sticker.face) == face && down[*place]
            });
            let mut colors = on_face.map(|(place, _)| solved[state[place] as usize]);
            let first = colors.next();
            first.is_some() && colors.all(|color| Some(color) == first)
        })
        .count()
}

/// Names the shape the top's corners make, from which way each one is twisted.
fn corner_shape(model: &StickerModel, state: &[u16], solved: &[u16]) -> &'static str {
    let at = |x: f64, z: f64, face: &str| {
        let wanted = model.face_names.iter().position(|name| *name == face);
        let wanted = wanted.expect("A cube has these faces") as u8;
        let place = model.stickers.iter().position(|sticker| {
            sticker.face == wanted
                && sticker.center[1] > 0.1
                && sticker.center[0] * x > 0.1
                && sticker.center[2] * z > 0.1
        });
        let place = place.expect("A corner has three stickers");
        solved[state[place] as usize] == 0
    };
    // Clockwise from above, each corner with the stickers clockwise of top.
    let corners = [
        (at(1., 1., "U"), at(1., 1., "F"), at(1., 1., "R")),
        (at(-1., 1., "U"), at(-1., 1., "L"), at(-1., 1., "F")),
        (at(-1., -1., "U"), at(-1., -1., "B"), at(-1., -1., "L")),
        (at(1., -1., "U"), at(1., -1., "R"), at(1., -1., "B")),
    ];
    let twist: Vec<u8> = corners
        .iter()
        .map(|&(top, first, _)| {
            if top {
                0
            } else if first {
                1
            } else {
                2
            }
        })
        .collect();
    let smallest = (0..4)
        .map(|shift| {
            let mut turned = twist.clone();
            turned.rotate_left(shift);
            turned
        })
        .min()
        .unwrap_or_default();
    match smallest.as_slice() {
        [0, 0, 0, 0] => "Solved",
        [0, 1, 1, 1] => "Sune",
        [0, 2, 2, 2] => "Antisune",
        [1, 2, 1, 2] => "H",
        [1, 1, 2, 2] => "Pi",
        [0, 0, 1, 2] => "U",
        [0, 0, 2, 1] => "T",
        [0, 1, 0, 2] => "L",
        _ => "Other",
    }
}

/// How awkward an algorithm is to do: the lower the better.
fn alg_cost(moves: &[u8], steps: &[Step]) -> u32 {
    moves.iter().map(|&step| steps[step as usize].cost).sum()
}

/// Inverts a sequence: reversed, each step swapped for its opposite power.
fn inverse_step(id: u8, steps: &[Step]) -> u8 {
    let step = &steps[id as usize];
    let powers: Vec<usize> = (0..steps.len())
        .filter(|&other| steps[other].family == step.family)
        .collect();
    let place = powers.iter().position(|&other| other == id as usize);
    let place = place.expect("A step is among its family");
    powers[powers.len() - 1 - place] as u8
}

fn text_of(moves: &[u8], steps: &[Step]) -> String {
    let texts: Vec<&str> = moves.iter().map(|&id| steps[id as usize].text.as_str()).collect();
    texts.join(" ")
}

/// Searches one set, printing its block.
fn search(spec: &Spec) -> String {
    let event = event_named(&spec.event);
    let model = Model::of(event);
    let Model::Stickers(stickers) = model else {
        panic!("{} is not a puzzle of stickers", spec.id);
    };
    let keep = model.region(&spec.before).expect("before");
    let steps = steps(stickers, &spec.moves);
    let numbering = model.numbered().expect("numbered");
    // Stickers outside `after` look alike, so that they can't tell cases apart.
    let after = model.region(&spec.after).expect("after");
    let mut colors = model.solved();
    for (color, chosen) in colors.iter_mut().zip(&after.places) {
        if !chosen {
            *color = u16::MAX;
        }
    }
    let turns = free_turns(stickers, &spec.turn);
    let down = model.region("layer:D").map(|region| region.places);
    let solved = model.solved();

    // Each sequence, as what it does to a numbered puzzle.
    let walked = walk(&numbering, &steps, spec.depth);
    eprintln!("{}: {} sequences", spec.id, walked.len());

    // A sequence A and the inverse of B leave the kept stickers alone exactly
    // when A and B send them to the same places. Sequences come shortest
    // first, so a full group only loses the longest.
    let mut halves: HashMap<Vec<u16>, Vec<u32>> = HashMap::new();
    let mut place_of = vec![0; numbering.len()];
    for (index, found) in walked.iter().enumerate() {
        for (place, &sticker) in found.state.iter().enumerate() {
            place_of[sticker as usize] = place as u16;
        }
        let mut key = masked(&place_of, &keep);
        if spec.loose {
            key.sort_unstable();
        }
        let group = halves.entry(key).or_default();
        if group.len() < GROUP_LIMIT {
            group.push(index as u32);
        }
    }
    let free: Vec<u16> = spec
        .turn
        .split_whitespace()
        .filter_map(|name| stickers.base(name))
        .collect();

    // The best algorithms of each case, cheapest first.
    let mut cases: HashMap<State, Vec<(u32, Vec<u8>)>> = HashMap::new();
    let solved_key = case_key(&numbering, &turns, &numbering, &colors);
    for group in halves.values() {
        for (at, &first) in group.iter().enumerate() {
            for &second in &group[at..] {
                let (a, b) = (&walked[first as usize], &walked[second as usize]);
                for (one, other) in [(a, b), (b, a)] {
                    let mut moves = one.moves.clone();
                    moves.extend(other.moves.iter().rev().map(|&id| inverse_step(id, &steps)));
                    let free_end = |id: Option<&u8>| {
                        id.is_some_and(|&id| free.contains(&steps[id as usize].family))
                    };
                    if moves.is_empty()
                        || moves.len() > spec.longest
                        || free_end(moves.first())
                        || free_end(moves.last())
                        || moves.windows(2).any(|pair| {
                            steps[pair[0] as usize].family == steps[pair[1] as usize].family
                        })
                    {
                        continue;
                    }
                    let mut state = numbering.clone();
                    for &id in &moves {
                        state = permuted(&state, &steps[id as usize].source);
                    }
                    if let Some(wanted) = spec.bars {
                        let down = down.as_ref().expect("A cube has a bottom");
                        if bars(stickers, &state, &solved, down) != wanted {
                            continue;
                        }
                    }
                    let key = case_key(&state, &turns, &numbering, &colors);
                    if key == solved_key {
                        continue;
                    }
                    let list = cases.entry(key).or_default();
                    let cost = alg_cost(&moves, &steps);
                    if list.len() < spec.keep || cost < list.last().map_or(u32::MAX, |best| best.0) {
                        if !list.iter().any(|known| known.1 == moves) {
                            list.push((cost, moves));
                            list.sort_by_key(|known| known.0);
                            list.truncate(spec.keep);
                        }
                    }
                }
            }
        }
    }

    let mut ordered: Vec<(State, Vec<(u32, Vec<u8>)>)> = cases.into_iter().collect();
    ordered.sort_by(|a, b| a.1[0].0.cmp(&b.1[0].0).then_with(|| a.0.cmp(&b.0)));
    eprintln!("{}: {} cases", spec.id, ordered.len());

    let mut block = String::new();
    let _ = writeln!(block, "\nset {}\nname {}\nmethod {}", spec.id, spec.name, spec.method);
    let _ = writeln!(block, "before {}\nafter {}\nturn {}", spec.before, spec.after, spec.turn);
    let _ = writeln!(block, "show {}\nview {}\nsource generated", spec.show, spec.view);
    let shape_of = |moves: &[u8]| {
        let mut state = numbering.clone();
        for &id in moves.iter().rev() {
            let inverse = inverse_step(id, &steps);
            state = permuted(&state, &steps[inverse as usize].source);
        }
        corner_shape(stickers, &state, &solved)
    };
    let mut lines: Vec<(usize, String)> = Vec::new();
    let mut counts: HashMap<&'static str, usize> = HashMap::new();
    for (number, (_, algorithms)) in ordered.iter().enumerate() {
        let texts: Vec<String> = algorithms
            .iter()
            .map(|(_, moves)| text_of(moves, &steps))
            .collect();
        let shape = if spec.shapes {
            shape_of(&algorithms[0].1)
        } else {
            ""
        };
        let count = counts.entry(shape).or_default();
        *count += 1;
        let (order, name) = if spec.shapes {
            let order = SHAPES.iter().position(|known| *known == shape).unwrap_or(9);
            (order, format!("{} {shape} {count}", spec.prefix))
        } else {
            (0, format!("{} {}", spec.prefix, number + 1))
        };
        let group = if spec.shapes {
            format!("group {shape}\n")
        } else {
            String::new()
        };
        lines.push((order, format!("{group}case {name} |  | {}\n", texts.join(" | "))));
    }
    lines.sort_by_key(|line| line.0);
    let mut last = String::new();
    for (_, line) in lines {
        match line.split_once('\n') {
            Some((group, rest)) if group.starts_with("group") && rest.starts_with("case") => {
                if group != last {
                    let _ = writeln!(block, "{group}");
                    last = group.to_owned();
                }
                block.push_str(rest);
            }
            _ => block.push_str(&line),
        }
    }
    block
}

fn main() {
    let path = std::env::args().nth(1).expect("Give a spec file");
    let text = std::fs::read_to_string(path).expect("Read the spec file");
    for spec in read_specs(&text) {
        print!("{}", search(&spec));
    }
}
