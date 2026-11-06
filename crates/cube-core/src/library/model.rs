//! State models of the puzzles, each with its own notation.
//!
//! A state is a list of small numbers. Most puzzles are solids cut into
//! stickers, and a move is a permutation of those. Square-1 keeps its 24 wedges
//! and Clock its 18 dials, because neither is a fixed permutation of stickers:
//! a Square-1 `/` can be blocked and Clock's dials add up.

use std::collections::HashMap;
use std::f64::consts::TAU;
use std::sync::LazyLock;

use super::geometry::{
    Plane, Vec3, centroid, cut, dot, face_polygon, normalized, rotated, same, scale,
};
use crate::event::WcaEvent;
use crate::trainer::notation::AlgError;

/// A puzzle's position: the sticker at each place, a Square-1's wedges and the
/// state of its middle layer, or a Clock's dials.
pub type State = Vec<u16>;

/// One move of an algorithm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// A move of a sticker puzzle, repeated `power` times.
    Turn { base: u16, power: u8 },
    /// Turns both Square-1 layers clockwise, in twelfths.
    Layers { top: i8, bottom: i8 },
    /// The Square-1 `/`.
    Slash,
    /// Turns a Clock's gears with the pins of `turn` up.
    Dial { turn: u8, amount: i8 },
    /// Turns a Clock over.
    Flip,
}

/// A piece of a solid: where it sits and which face it began on.
#[derive(Clone, Debug)]
pub struct Sticker {
    pub face: u8,
    pub center: Vec3,
    pub normal: Vec3,
    pub polygon: Vec<Vec3>,
}

/// A move of a sticker puzzle.
struct Move {
    name: String,
    /// Which place each place takes its sticker from.
    source: Vec<u16>,
    /// How many times the move must be done to do nothing.
    order: u8,
    /// Which places it moves, counting those that only spin in place.
    turned: Vec<bool>,
    /// Whether it turns the whole puzzle.
    rotation: bool,
}

/// A puzzle of stickers.
pub struct StickerModel {
    pub stickers: Vec<Sticker>,
    pub face_names: Vec<&'static str>,
    moves: Vec<Move>,
    by_name: HashMap<String, u16>,
    /// Move names, longest first, so that `Rw` is not read as `R` and `w`.
    names: Vec<String>,
    /// Every way of holding the puzzle, as whole-puzzle permutations.
    holds: Vec<Vec<u16>>,
    /// The moves of the faces, which tell a corner from an edge.
    face_moves: Vec<u16>,
}

/// A move to build from a plane through the solid.
struct TurnSpec {
    name: String,
    axis: Vec3,
    /// The stickers from just above `from` to `to` turn, along the axis.
    from: f64,
    to: f64,
    /// A fraction of a full turn, clockwise as seen from the tip of the axis.
    order: u8,
    rotation: bool,
    face_move: bool,
}

impl StickerModel {
    fn build(faces: &[(&'static str, Vec3, f64)], cuts: &[Plane], specs: Vec<TurnSpec>) -> Self {
        let planes: Vec<Plane> = faces
            .iter()
            .map(|&(_, normal, offset)| Plane { normal, offset })
            .collect();
        let mut stickers = Vec::new();
        for (index, plane) in planes.iter().enumerate() {
            for polygon in cut(face_polygon(&planes, index), cuts) {
                stickers.push(Sticker {
                    face: index as u8,
                    center: centroid(&polygon),
                    normal: plane.normal,
                    polygon,
                });
            }
        }
        let mut model = Self {
            stickers,
            face_names: faces.iter().map(|face| face.0).collect(),
            moves: Vec::new(),
            by_name: HashMap::new(),
            names: Vec::new(),
            holds: Vec::new(),
            face_moves: Vec::new(),
        };
        for spec in specs {
            model.add(&spec);
        }
        model.names = model.by_name.keys().cloned().collect();
        model
            .names
            .sort_by_key(|name| std::cmp::Reverse(name.len()));
        model.holds = model.all_holds();
        model
    }

    fn add(&mut self, spec: &TurnSpec) {
        let angle = -TAU / f64::from(spec.order);
        let count = self.stickers.len();
        let mut source: Vec<u16> = (0..count as u16).collect();
        let mut turned = vec![false; count];
        for (index, sticker) in self.stickers.iter().enumerate() {
            let height = dot(spec.axis, sticker.center);
            if height <= spec.from || height > spec.to {
                continue;
            }
            turned[index] = true;
            let center = rotated(sticker.center, spec.axis, angle);
            let normal = rotated(sticker.normal, spec.axis, angle);
            let target = self
                .stickers
                .iter()
                .position(|other| same(other.center, center) && same(other.normal, normal));
            let target = target.unwrap_or_else(|| panic!("{} leaves the puzzle", spec.name));
            source[target] = index as u16;
        }
        let id = self.moves.len() as u16;
        self.moves.push(Move {
            name: spec.name.clone(),
            source,
            order: spec.order,
            turned,
            rotation: spec.rotation,
        });
        self.by_name.insert(spec.name.clone(), id);
        if spec.face_move {
            self.face_moves.push(id);
        }
    }

    /// Builds every whole-puzzle position from the rotation moves.
    fn all_holds(&self) -> Vec<Vec<u16>> {
        let identity: Vec<u16> = (0..self.stickers.len() as u16).collect();
        let mut holds = vec![identity];
        let mut next = 0;
        while next < holds.len() {
            for turn in self.moves.iter().filter(|turn| turn.rotation) {
                let moved: Vec<u16> = turn
                    .source
                    .iter()
                    .map(|&from| holds[next][from as usize])
                    .collect();
                if !holds.contains(&moved) {
                    holds.push(moved);
                }
            }
            next += 1;
        }
        holds
    }

    fn turn(&self, state: &mut State, base: u16, power: u8) {
        let turn = &self.moves[base as usize];
        for _ in 0..power {
            let old = state.clone();
            for (place, &from) in turn.source.iter().enumerate() {
                state[place] = old[from as usize];
            }
        }
    }

    /// Reads the moves of `text`, which need no spaces between them.
    fn parse(&self, text: &str) -> Result<Vec<Op>, AlgError> {
        let is_gap = |letter: char| letter.is_whitespace() || "()[],".contains(letter);
        let mut ops = Vec::new();
        let mut rest = text;
        loop {
            rest = rest.trim_start_matches(is_gap);
            if rest.is_empty() {
                break;
            }
            let invalid = |from: &str| {
                let end = from.find(is_gap).unwrap_or(from.len());
                AlgError::InvalidMove(from[..end].to_owned())
            };
            let known = self.names.iter().find(|name| rest.starts_with(name.as_str()));
            let Some(name) = known else {
                return Err(invalid(rest));
            };
            let base = self.by_name[name];
            let order = self.moves[base as usize].order;
            let mut tail = &rest[name.len()..];
            let mut power = 1;
            if let Some(digit) = tail.chars().next().and_then(|digit| digit.to_digit(10)) {
                if digit == 0 || digit >= u32::from(order) {
                    return Err(invalid(rest));
                }
                power = digit as u8;
                tail = &tail[1..];
            }
            if let Some(prime) = tail.chars().next().filter(|mark| "'’`".contains(*mark)) {
                power = order - power;
                tail = &tail[prime.len_utf8()..];
            }
            ops.push(Op::Turn { base, power });
            rest = tail;
        }
        Ok(ops)
    }

    /// Returns the name of a move, written the usual way.
    pub fn text(&self, base: u16, power: u8) -> String {
        let turn = &self.moves[base as usize];
        let name = &turn.name;
        if power == 1 {
            name.clone()
        } else if power == turn.order - 1 {
            format!("{name}'")
        } else if power * 2 > turn.order {
            format!("{name}{}'", turn.order - power)
        } else {
            format!("{name}{power}")
        }
    }

    /// Returns the id of the move called `name`.
    pub fn base(&self, name: &str) -> Option<u16> {
        self.by_name.get(name).copied()
    }

    /// Returns how many times a move can be repeated before it does nothing.
    pub fn order(&self, base: u16) -> u8 {
        self.moves[base as usize].order
    }

    /// Returns which place each place takes its sticker from, after a move.
    pub fn source(&self, base: u16) -> &[u16] {
        &self.moves[base as usize].source
    }

    /// Returns the stickers a term of a region names, such as `layer:U`.
    fn atom(&self, atom: &str) -> Option<Vec<bool>> {
        let count = self.stickers.len();
        if atom == "all" {
            return Some(vec![true; count]);
        }
        let (kind, value) = atom.split_once(':')?;
        match kind {
            "layer" => {
                let base = *self.by_name.get(value)?;
                Some(self.moves[base as usize].turned.clone())
            }
            "face" => {
                let face = self.face_names.iter().position(|name| *name == value)?;
                Some(
                    self.stickers
                        .iter()
                        .map(|sticker| usize::from(sticker.face) == face)
                        .collect(),
                )
            }
            // How far a sticker sits towards a face: `above:D:0.3`.
            "above" => {
                let (face, height) = value.split_once(':')?;
                let height: f64 = height.parse().ok()?;
                let face = self.face_names.iter().position(|name| *name == face)?;
                let normal = self.stickers.iter().find(|sticker| usize::from(sticker.face) == face)?;
                let normal = normal.normal;
                let stickers = self.stickers.iter();
                Some(
                    stickers
                        .map(|sticker| dot(normal, sticker.center) > height)
                        .collect(),
                )
            }
            "touching" => {
                let wanted: usize = value.parse().ok()?;
                let touched = |place: usize| {
                    let turns = self.face_moves.iter();
                    turns
                        .filter(|&&turn| self.moves[turn as usize].turned[place])
                        .count()
                };
                Some((0..count).map(|place| touched(place) == wanted).collect())
            }
            _ => None,
        }
    }
}

/// Square-1's wedges, as `TNoodle` numbers them. Slot 24 is the middle layer.
const SQUARE1_SOLVED: [u16; 25] = [
    0, 0, 1, 2, 2, 3, 4, 4, 5, 6, 6, 7, 8, 9, 9, 10, 11, 11, 12, 13, 13, 14, 15, 15, 0,
];

/// Returns whether a Square-1 piece is a corner, which fills two slots.
pub const fn square1_corner(piece: u16) -> bool {
    matches!(piece, 0 | 2 | 4 | 6 | 9 | 11 | 13 | 15)
}

/// Returns whether a Square-1 can do a `/`.
pub fn square1_can_slash(state: &[u16]) -> bool {
    state[0] != state[11] && state[6] != state[5] && state[12] != state[23] && state[18] != state[17]
}

/// Returns whether a Square-1's pieces are an even permutation of the solved
/// order, reading each layer from slot 0.
fn square1_even(state: &[u16]) -> bool {
    let mut pieces: State = Vec::with_capacity(16);
    for &piece in &state[..24] {
        if pieces.last() != Some(&piece) {
            pieces.push(piece);
        }
    }
    let mut swaps = usize::from(state[24] != 0);
    for first in 0..pieces.len() {
        for second in first + 1..pieces.len() {
            swaps += usize::from(pieces[first] > pieces[second]);
        }
    }
    swaps % 2 == 0
}

/// The pins of each Clock turn, in `TNoodle`'s order.
pub const CLOCK_TURNS: [&str; 9] = ["UR", "DR", "DL", "UL", "U", "R", "D", "L", "ALL"];

/// Which dials each pin configuration turns: the front nine, then the back.
const CLOCK_MOVES: [[i8; 18]; 9] = [
    [0, 1, 1, 0, 1, 1, 0, 0, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 1, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, -1, 0, 0],
    [0, 0, 0, 1, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, -1],
    [1, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, -1, 0, 0, 0, 0, 0, 0],
    [1, 1, 1, 1, 1, 1, 0, 0, 0, -1, 0, -1, 0, 0, 0, 0, 0, 0],
    [0, 1, 1, 0, 1, 1, 0, 1, 1, -1, 0, 0, 0, 0, 0, -1, 0, 0],
    [0, 0, 0, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, -1, 0, -1],
    [1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 0, -1, 0, 0, 0, 0, 0, -1],
    [1, 1, 1, 1, 1, 1, 1, 1, 1, -1, 0, -1, 0, 0, 0, -1, 0, -1],
];

/// A set of places to compare with the solved puzzle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    pub places: Vec<bool>,
    /// A Square-1 only asks for the cube shape, not which piece is where.
    pub shape_only: bool,
    /// A Square-1 must also have an even permutation.
    pub even: bool,
}

/// A puzzle's rules.
pub enum Model {
    Stickers(StickerModel),
    Square1,
    Clock,
}

impl Model {
    /// Returns the model of `event`, building it the first time.
    pub fn of(event: WcaEvent) -> &'static Self {
        static CUBES: [LazyLock<Model>; 6] = [
            LazyLock::new(|| Model::Stickers(cube(2))),
            LazyLock::new(|| Model::Stickers(cube(3))),
            LazyLock::new(|| Model::Stickers(cube(4))),
            LazyLock::new(|| Model::Stickers(cube(5))),
            LazyLock::new(|| Model::Stickers(cube(6))),
            LazyLock::new(|| Model::Stickers(cube(7))),
        ];
        static MEGAMINX: LazyLock<Model> = LazyLock::new(|| Model::Stickers(megaminx()));
        static PYRAMINX: LazyLock<Model> = LazyLock::new(|| Model::Stickers(pyraminx()));
        static SKEWB: LazyLock<Model> = LazyLock::new(|| Model::Stickers(skewb()));
        static FTO: LazyLock<Model> = LazyLock::new(|| Model::Stickers(fto()));
        static SQUARE1: Model = Model::Square1;
        static CLOCK: Model = Model::Clock;
        match event {
            WcaEvent::Cube2x2 => &CUBES[0],
            WcaEvent::Cube3x3 | WcaEvent::Cube3x3OneHanded | WcaEvent::Cube3x3Blind => &CUBES[1],
            WcaEvent::Cube4x4 | WcaEvent::Cube4x4Blind => &CUBES[2],
            WcaEvent::Cube5x5 | WcaEvent::Cube5x5Blind => &CUBES[3],
            WcaEvent::Cube6x6 => &CUBES[4],
            WcaEvent::Cube7x7 => &CUBES[5],
            WcaEvent::Megaminx => &MEGAMINX,
            WcaEvent::Pyraminx => &PYRAMINX,
            WcaEvent::Skewb => &SKEWB,
            WcaEvent::Fto => &FTO,
            WcaEvent::Square1 => &SQUARE1,
            WcaEvent::Clock => &CLOCK,
        }
    }

    /// Returns the solved puzzle.
    pub fn solved(&self) -> State {
        match self {
            Self::Stickers(model) => model
                .stickers
                .iter()
                .map(|sticker| u16::from(sticker.face))
                .collect(),
            Self::Square1 => SQUARE1_SOLVED.to_vec(),
            Self::Clock => vec![0; 18],
        }
    }

    /// Returns a state that tells every place apart, to follow where each
    /// sticker goes. Only a puzzle of stickers has one.
    pub fn numbered(&self) -> Option<State> {
        match self {
            Self::Stickers(model) => Some((0..model.stickers.len() as u16).collect()),
            Self::Square1 | Self::Clock => None,
        }
    }

    /// Reads an algorithm in the puzzle's notation.
    pub fn parse(&self, text: &str) -> Result<Vec<Op>, AlgError> {
        let ops = match self {
            Self::Stickers(model) => model.parse(text)?,
            Self::Square1 => parse_square1(text)?,
            Self::Clock => parse_clock(text)?,
        };
        if ops.is_empty() {
            return Err(AlgError::Empty);
        }
        Ok(ops)
    }

    /// Does `ops` to `state`. Returns `false`, leaving a partly changed state,
    /// when a Square-1 is blocked.
    pub fn apply(&self, state: &mut State, ops: &[Op]) -> bool {
        for &op in ops {
            match (self, op) {
                (Self::Stickers(model), Op::Turn { base, power }) => {
                    model.turn(state, base, power);
                }
                (Self::Square1, Op::Layers { top, bottom }) => {
                    let top = (-i32::from(top)).rem_euclid(12) as usize;
                    let bottom = (-i32::from(bottom)).rem_euclid(12) as usize;
                    state[..12].rotate_left(top);
                    state[12..24].rotate_left(bottom);
                }
                (Self::Square1, Op::Slash) => {
                    if !square1_can_slash(state) {
                        return false;
                    }
                    for index in 0..6 {
                        state.swap(index + 12, index + 6);
                    }
                    state[24] ^= 1;
                }
                (Self::Clock, Op::Dial { turn, amount }) => {
                    for (dial, step) in state.iter_mut().zip(CLOCK_MOVES[turn as usize]) {
                        let moved = *dial as i16 + i16::from(step) * i16::from(amount);
                        *dial = moved.rem_euclid(12) as u16;
                    }
                }
                (Self::Clock, Op::Flip) => state.rotate_left(9),
                _ => {}
            }
        }
        true
    }

    /// Returns the moves that undo `ops`.
    pub fn inverse(&self, ops: &[Op]) -> Vec<Op> {
        let undone = |op: &Op| match *op {
            Op::Turn { base, power } => {
                let Self::Stickers(model) = self else {
                    return *op;
                };
                let order = model.moves[base as usize].order;
                Op::Turn {
                    base,
                    power: (order - power) % order,
                }
            }
            Op::Layers { top, bottom } => Op::Layers {
                top: -top,
                bottom: -bottom,
            },
            Op::Dial { turn, amount } => Op::Dial {
                turn,
                amount: -amount,
            },
            Op::Slash | Op::Flip => *op,
        };
        ops.iter().rev().map(undone).collect()
    }

    /// Reads a region, such as `all -layer:U`: each word adds what it names,
    /// `-` takes it away, and `&` keeps what two names have in common.
    pub fn region(&self, text: &str) -> Option<Region> {
        let size = self.solved().len();
        let mut region = Region {
            places: vec![false; size],
            shape_only: false,
            even: false,
        };
        for word in text.split_whitespace() {
            let (removing, word) = word
                .strip_prefix('-')
                .map_or((false, word), |word| (true, word));
            let mut found = vec![true; size];
            for atom in word.split('&') {
                let places = self.atom(atom, &mut region)?;
                for (kept, place) in found.iter_mut().zip(places) {
                    *kept &= place;
                }
            }
            for (place, chosen) in region.places.iter_mut().zip(found) {
                if chosen {
                    *place = !removing;
                }
            }
        }
        Some(region)
    }

    fn atom(&self, atom: &str, region: &mut Region) -> Option<Vec<bool>> {
        match self {
            Self::Stickers(model) => model.atom(atom),
            Self::Square1 => {
                let slots = |test: &dyn Fn(u16) -> bool| {
                    let solved = SQUARE1_SOLVED.iter().take(24);
                    let mut places: Vec<bool> = solved.map(|&piece| test(piece)).collect();
                    places.push(false);
                    places
                };
                match atom {
                    "all" => {
                        let mut places = slots(&|_| true);
                        places[24] = true;
                        Some(places)
                    }
                    "corners" => Some(slots(&square1_corner)),
                    "edges" => Some(slots(&|piece| !square1_corner(piece))),
                    "shape" => {
                        region.shape_only = true;
                        Some(slots(&|_| true))
                    }
                    "even" => {
                        region.even = true;
                        Some(vec![false; 25])
                    }
                    _ => None,
                }
            }
            Self::Clock => {
                let dials: &[usize] = match atom {
                    "all" => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17],
                    "front" => &[0, 1, 2, 3, 4, 5, 6, 7, 8],
                    "back" => &[9, 10, 11, 12, 13, 14, 15, 16, 17],
                    "front-cross" => &[1, 3, 4, 5, 7],
                    "back-cross" => &[10, 12, 13, 14, 16],
                    "corners" => &[0, 2, 6, 8],
                    _ => return None,
                };
                let mut places = vec![false; 18];
                for &dial in dials {
                    places[dial] = true;
                }
                Some(places)
            }
        }
    }

    /// Returns whether the places of `region` look solved.
    pub fn matches(&self, state: &[u16], region: &Region) -> bool {
        let solved = self.solved();
        let places = state.iter().zip(&solved).zip(&region.places);
        let mut wanted = places.filter(|(_, chosen)| **chosen).map(|(pair, _)| pair);
        let fits = if region.shape_only {
            wanted.all(|(&place, &home)| square1_corner(place) == square1_corner(home))
        } else if matches!(self, Self::Clock) {
            // A Clock is solved with all the chosen dials alike.
            let first = region.places.iter().position(|chosen| *chosen);
            let reference = first.map_or(0, |place| state[place]);
            wanted.all(|(&place, _)| place == reference)
        } else {
            wanted.all(|(place, home)| place == home)
        };
        fits && (!region.even || square1_even(state))
    }

    /// Returns `state` seen in every way of holding the puzzle.
    pub fn holds(&self, state: &[u16]) -> Vec<State> {
        match self {
            Self::Stickers(model) => model
                .holds
                .iter()
                .map(|hold| hold.iter().map(|&from| state[from as usize]).collect())
                .collect(),
            Self::Square1 | Self::Clock => vec![state.to_vec()],
        }
    }
}

/// Reads a Square-1 algorithm such as `(1,0)/ (3,0)/ (-1,-1)`.
fn parse_square1(text: &str) -> Result<Vec<Op>, AlgError> {
    let mut ops = Vec::new();
    let mut rest = text;
    loop {
        rest = rest.trim_start_matches(|letter: char| letter.is_whitespace() || letter == '(');
        if rest.is_empty() {
            return Ok(ops);
        }
        if let Some(tail) = rest.strip_prefix('/') {
            ops.push(Op::Slash);
            rest = tail;
            continue;
        }
        let end = rest.find(['/', ')']).unwrap_or(rest.len());
        let invalid = || AlgError::InvalidMove(rest[..end].trim().to_owned());
        let (top, bottom) = rest[..end].split_once(',').ok_or_else(invalid)?;
        let top: i8 = top.trim().parse().map_err(|_| invalid())?;
        let bottom: i8 = bottom.trim().parse().map_err(|_| invalid())?;
        ops.push(Op::Layers { top, bottom });
        rest = rest[end..].trim_start_matches(')');
    }
}

/// Reads a Clock algorithm such as `UR3+ ALL2- y2 U1+`.
fn parse_clock(text: &str) -> Result<Vec<Op>, AlgError> {
    let mut ops = Vec::new();
    for word in text.split_whitespace() {
        let invalid = || AlgError::InvalidMove(word.to_owned());
        if word == "y2" {
            ops.push(Op::Flip);
            continue;
        }
        let digits = word.find(|letter: char| letter.is_ascii_digit());
        let (name, amount) = word.split_at(digits.ok_or_else(invalid)?);
        let turn = CLOCK_TURNS.iter().position(|turn| *turn == name);
        let turn = turn.ok_or_else(invalid)? as u8;
        let (amount, sign) = amount.split_at(amount.len() - 1);
        let amount: i8 = amount.parse().map_err(|_| invalid())?;
        let amount = match sign {
            "+" => amount,
            "-" => -amount,
            _ => return Err(invalid()),
        };
        ops.push(Op::Dial { turn, amount });
    }
    Ok(ops)
}

fn turn(name: impl Into<String>, axis: Vec3, from: f64, to: f64, order: u8) -> TurnSpec {
    TurnSpec {
        name: name.into(),
        axis,
        from,
        to,
        order,
        rotation: false,
        face_move: false,
    }
}

/// The three whole-puzzle rotations of anything held like a cube.
fn cube_rotations() -> Vec<TurnSpec> {
    [
        ("x", [1., 0., 0.]),
        ("y", [0., 1., 0.]),
        ("z", [0., 0., 1.]),
    ]
    .into_iter()
    .map(|(name, axis)| TurnSpec {
        rotation: true,
        ..turn(name, axis, -2., 2., 4)
    })
    .collect()
}

/// Builds a cube of `size` layers: `x`, `y`, `z`, face turns, `Rw` and `3Rw`
/// wide turns with lower case letters for `Rw`, `2R` inner layers, and on odd
/// cubes the `M`, `E` and `S` middle slices.
fn cube(size: usize) -> StickerModel {
    const FACES: [(&str, Vec3); 6] = [
        ("U", [0., 1., 0.]),
        ("R", [1., 0., 0.]),
        ("F", [0., 0., 1.]),
        ("D", [0., -1., 0.]),
        ("L", [-1., 0., 0.]),
        ("B", [0., 0., -1.]),
    ];
    let layer = 2. / size as f64;
    let faces: Vec<(&'static str, Vec3, f64)> = FACES
        .iter()
        .map(|&(name, normal)| (name, normal, 1.))
        .collect();
    let mut cuts = Vec::new();
    for normal in [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]] {
        for depth in 1..size {
            cuts.push(Plane {
                normal,
                offset: 1. - layer * depth as f64,
            });
        }
    }
    let mut specs = Vec::new();
    for (name, axis) in FACES {
        let top = |depth: usize| 1. - layer * depth as f64 + 1e-9;
        specs.push(TurnSpec {
            face_move: true,
            ..turn(name, axis, top(1), 2., 4)
        });
        for depth in 2..size {
            let wide = if depth == 2 {
                format!("{name}w")
            } else {
                format!("{depth}{name}w")
            };
            specs.push(turn(wide, axis, top(depth), 2., 4));
            specs.push(turn(
                format!("{depth}{name}"),
                axis,
                top(depth),
                top(depth - 1),
                4,
            ));
            if depth == 2 {
                specs.push(turn(format!("2{name}w"), axis, top(2), 2., 4));
                specs.push(turn(name.to_lowercase(), axis, top(2), 2., 4));
            }
        }
    }
    if size % 2 == 1 {
        let half = layer / 2.;
        for (name, axis) in [
            ("M", [-1., 0., 0.]),
            ("E", [0., -1., 0.]),
            ("S", [0., 0., 1.]),
        ] {
            specs.push(turn(name, axis, -half, half, 4));
        }
    }
    specs.extend(cube_rotations());
    StickerModel::build(&faces, &cuts, specs)
}

/// Builds a Pyraminx in the WCA hold: `U L R B` turn two layers, and lower case
/// turns only the tip.
fn pyraminx() -> StickerModel {
    let root = 1. / 3_f64.sqrt();
    let corners: [(&str, Vec3); 4] = [
        ("U", [root, root, root]),
        ("L", [-root, -root, root]),
        ("R", [root, -root, -root]),
        ("B", [-root, root, -root]),
    ];
    // Each face is named for what it shows, opposite one of the corners.
    let names = ["D", "R", "L", "F"];
    let faces: Vec<(&'static str, Vec3, f64)> = corners
        .iter()
        .zip(names)
        .map(|(&(_, corner), name)| (name, scale(corner, -1.), 1. / 3.))
        .collect();
    let mut cuts = Vec::new();
    let mut specs = Vec::new();
    for (name, axis) in corners {
        for offset in [1. / 9., 5. / 9.] {
            cuts.push(Plane {
                normal: axis,
                offset,
            });
        }
        specs.push(TurnSpec {
            face_move: true,
            ..turn(name, axis, 1. / 9., 2., 3)
        });
        specs.push(turn(name.to_lowercase(), axis, 5. / 9., 2., 3));
    }
    StickerModel::build(&faces, &cuts, specs)
}

/// Builds a Skewb in WCA fixed corner notation, with `R`, `U`, `L` and `B`
/// turning the corners at `DFR`, `URB`, `DLB` and `DRB`.
fn skewb() -> StickerModel {
    let faces: Vec<(&'static str, Vec3, f64)> = vec![
        ("U", [0., 1., 0.], 1.),
        ("R", [1., 0., 0.], 1.),
        ("F", [0., 0., 1.], 1.),
        ("D", [0., -1., 0.], 1.),
        ("L", [-1., 0., 0.], 1.),
        ("B", [0., 0., -1.], 1.),
    ];
    let corners: [(&str, Vec3); 4] = [
        ("R", [1., -1., 1.]),
        ("U", [1., 1., -1.]),
        ("L", [-1., -1., -1.]),
        ("B", [1., -1., -1.]),
    ];
    let mut cuts = Vec::new();
    let mut specs = Vec::new();
    for (name, corner) in corners {
        let axis = normalized(corner);
        cuts.push(Plane {
            normal: axis,
            offset: 0.,
        });
        specs.push(TurnSpec {
            face_move: true,
            ..turn(name, axis, 0., 2., 3)
        });
    }
    specs.extend(cube_rotations());
    StickerModel::build(&faces, &cuts, specs)
}

/// Builds a Face Turning Octahedron with `TNoodle`'s faces.
fn fto() -> StickerModel {
    let faces_at: [(&'static str, Vec3); 8] = [
        ("U", [-1., 1., -1.]),
        ("R", [1., 1., -1.]),
        ("F", [1., 1., 1.]),
        ("L", [-1., 1., 1.]),
        ("B", [-1., -1., -1.]),
        ("BL", [-1., -1., 1.]),
        ("D", [1., -1., 1.]),
        ("BR", [1., -1., -1.]),
    ];
    let mut faces = Vec::new();
    let mut cuts = Vec::new();
    let mut specs = Vec::new();
    for (name, normal) in faces_at {
        let axis = normalized(normal);
        faces.push((name, axis, 1.));
        cuts.push(Plane {
            normal: axis,
            offset: 1. / 3.,
        });
        specs.push(TurnSpec {
            face_move: true,
            ..turn(name, axis, 1. / 3., 2., 3)
        });
    }
    StickerModel::build(&faces, &cuts, specs)
}

/// Builds a Megaminx. Five faces ring `U`, five more ring `D`, and each turns
/// by a fifth: `R`, `R2`, `R2'` and `R'`.
fn megaminx() -> StickerModel {
    // The angle between neighbouring faces of a dodecahedron, from the top.
    let tilt = (1. / 5_f64.sqrt()).acos();
    let ring = |degrees: f64, height: f64| -> Vec3 {
        let angle = f64::to_radians(degrees);
        let across = tilt.sin();
        [across * angle.cos(), height, -across * angle.sin()]
    };
    let upper = tilt.cos();
    let faces_at: [(&'static str, Vec3); 12] = [
        ("U", [0., 1., 0.]),
        ("F", ring(-90., upper)),
        ("R", ring(-18., upper)),
        ("BR", ring(54., upper)),
        ("BL", ring(126., upper)),
        ("L", ring(198., upper)),
        ("DR", ring(-54., -upper)),
        ("DBR", ring(18., -upper)),
        ("B", ring(90., -upper)),
        ("DBL", ring(162., -upper)),
        ("DL", ring(234., -upper)),
        ("D", [0., -1., 0.]),
    ];
    let depth = 0.62;
    let mut faces = Vec::new();
    let mut cuts = Vec::new();
    let mut specs = Vec::new();
    for (name, axis) in faces_at {
        faces.push((name, axis, 1.));
        cuts.push(Plane {
            normal: axis,
            offset: depth,
        });
        specs.push(TurnSpec {
            face_move: true,
            ..turn(name, axis, depth, 2., 5)
        });
    }
    StickerModel::build(&faces, &cuts, specs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(event: WcaEvent, algorithm: &str) -> State {
        let model = Model::of(event);
        let mut state = model.solved();
        assert!(model.apply(&mut state, &model.parse(algorithm).unwrap()));
        state
    }

    fn stickers(event: WcaEvent) -> usize {
        Model::of(event).solved().len()
    }

    #[test]
    fn every_solid_has_the_stickers_of_the_real_puzzle() {
        assert_eq!(stickers(WcaEvent::Cube2x2), 24);
        assert_eq!(stickers(WcaEvent::Cube3x3), 54);
        assert_eq!(stickers(WcaEvent::Cube4x4), 96);
        assert_eq!(stickers(WcaEvent::Cube5x5), 150);
        assert_eq!(stickers(WcaEvent::Cube6x6), 216);
        assert_eq!(stickers(WcaEvent::Cube7x7), 294);
        assert_eq!(stickers(WcaEvent::Pyraminx), 36);
        assert_eq!(stickers(WcaEvent::Skewb), 30);
        assert_eq!(stickers(WcaEvent::Fto), 72);
        assert_eq!(stickers(WcaEvent::Megaminx), 132);
    }

    /// Doing a move as many times as its order does nothing, and no fewer.
    #[test]
    fn moves_have_the_order_of_the_real_puzzle() {
        let cases = [
            (WcaEvent::Cube3x3, "R", 4),
            (WcaEvent::Cube3x3, "M", 4),
            (WcaEvent::Cube5x5, "3Rw", 4),
            (WcaEvent::Cube4x4, "2R", 4),
            (WcaEvent::Pyraminx, "U", 3),
            (WcaEvent::Pyraminx, "u", 3),
            (WcaEvent::Skewb, "R", 3),
            (WcaEvent::Fto, "BR", 3),
            (WcaEvent::Megaminx, "DBL", 5),
        ];
        for (event, name, order) in cases {
            let model = Model::of(event);
            for count in 1..=order {
                let state = run(event, &vec![name; count].join(" "));
                assert_eq!(state == model.solved(), count == order, "{name} x{count}");
            }
        }
    }

    /// Well-known orders pin down that turns go the right way round and that
    /// faces are where they should be.
    #[test]
    fn well_known_sequences_have_their_known_orders() {
        let order = |event: WcaEvent, algorithm: &str| {
            let model = Model::of(event);
            let ops = model.parse(algorithm).unwrap();
            let mut state = model.solved();
            let mut count = 0;
            loop {
                model.apply(&mut state, &ops);
                count += 1;
                if state == model.solved() {
                    return count;
                }
            }
        };
        assert_eq!(order(WcaEvent::Cube3x3, "R U"), 105);
        assert_eq!(order(WcaEvent::Cube3x3, "R U R' U'"), 6);
        assert_eq!(order(WcaEvent::Cube2x2, "R U"), 15);
    }

    #[test]
    fn a_rotation_is_every_layer_at_once() {
        assert_eq!(
            run(WcaEvent::Cube3x3, "x"),
            run(WcaEvent::Cube3x3, "R M' L'")
        );
        assert_eq!(run(WcaEvent::Cube3x3, "r"), run(WcaEvent::Cube3x3, "R M'"));
        assert_eq!(run(WcaEvent::Cube4x4, "Rw"), run(WcaEvent::Cube4x4, "R 2R"));
        assert_eq!(
            run(WcaEvent::Cube5x5, "3Rw"),
            run(WcaEvent::Cube5x5, "R 2R 3R")
        );
    }

    #[test]
    fn the_cube_agrees_with_the_solver_used_for_scrambles() {
        use tnoodle::min2phase::tools::from_scramble;
        const ORDER: &[u8; 6] = b"URFDLB";
        let scramble = "R U2 F' D L2 B R' U F2 D' L B2";
        let Model::Stickers(stickers) = Model::of(WcaEvent::Cube3x3) else {
            panic!("A cube is made of stickers");
        };
        let state = run(WcaEvent::Cube3x3, scramble);
        let facelets = from_scramble(scramble);
        // Each face, read from the top left, as the solver reads it.
        let reference: [(Vec3, Vec3, Vec3); 6] = [
            ([0., 1., 0.], [1., 0., 0.], [0., 0., 1.]),
            ([1., 0., 0.], [0., 0., -1.], [0., -1., 0.]),
            ([0., 0., 1.], [1., 0., 0.], [0., -1., 0.]),
            ([0., -1., 0.], [1., 0., 0.], [0., 0., -1.]),
            ([-1., 0., 0.], [0., 0., 1.], [0., -1., 0.]),
            ([0., 0., -1.], [-1., 0., 0.], [0., -1., 0.]),
        ];
        let third = 2. / 3.;
        for (face, (normal, right, down)) in reference.into_iter().enumerate() {
            for row in 0..3 {
                for column in 0..3 {
                    let across = (f64::from(column) - 1.) * third;
                    let below = (f64::from(row) - 1.) * third;
                    let at = [
                        normal[0] + right[0] * across + down[0] * below,
                        normal[1] + right[1] * across + down[1] * below,
                        normal[2] + right[2] * across + down[2] * below,
                    ];
                    let place = stickers
                        .stickers
                        .iter()
                        .position(|sticker| same(sticker.center, at))
                        .unwrap();
                    let expected = facelets.as_bytes()[face * 9 + (row * 3 + column) as usize];
                    assert_eq!(ORDER[state[place] as usize], expected);
                }
            }
        }
    }

    #[test]
    fn square1_blocks_a_slash_it_cannot_do() {
        let model = Model::of(WcaEvent::Square1);
        let mut state = model.solved();
        assert!(!model.apply(&mut state, &model.parse("(2,0)/").unwrap()));
        let mut state = model.solved();
        assert!(model.apply(&mut state, &model.parse("/ (3,0) /").unwrap()));
    }

    #[test]
    fn an_inverse_undoes_any_puzzle() {
        let cases = [
            (WcaEvent::Cube3x3, "r U R' F2 M' y L"),
            (WcaEvent::Megaminx, "R U2' F BL' DBR2"),
            (WcaEvent::Pyraminx, "U L' r b'"),
            (WcaEvent::Skewb, "R U' L B x"),
            (WcaEvent::Fto, "BR U' F BL"),
            (WcaEvent::Square1, "/ (3,3)/ (-1,2)"),
            (WcaEvent::Clock, "UR3+ y2 ALL2- L1+"),
        ];
        for (event, algorithm) in cases {
            let model = Model::of(event);
            let ops = model.parse(algorithm).unwrap();
            let mut state = model.solved();
            assert!(model.apply(&mut state, &ops), "{algorithm}");
            assert_ne!(state, model.solved(), "{algorithm}");
            assert!(model.apply(&mut state, &model.inverse(&ops)), "{algorithm}");
            assert_eq!(state, model.solved(), "{algorithm}");
        }
    }

    #[test]
    fn notation_errors_name_the_move() {
        let cube = Model::of(WcaEvent::Cube3x3);
        assert_eq!(
            cube.parse("R U Q"),
            Err(AlgError::InvalidMove("Q".to_owned()))
        );
        assert_eq!(
            cube.parse("R5"),
            Err(AlgError::InvalidMove("R5".to_owned()))
        );
        assert_eq!(cube.parse(" ( ) "), Err(AlgError::Empty));
        let clock = Model::of(WcaEvent::Clock);
        assert_eq!(
            clock.parse("UR3"),
            Err(AlgError::InvalidMove("UR3".to_owned()))
        );
    }

    #[test]
    fn regions_add_and_take_away() {
        let model = Model::of(WcaEvent::Cube3x3);
        let count = |text: &str| {
            let region = model.region(text).unwrap();
            region.places.iter().filter(|chosen| **chosen).count()
        };
        assert_eq!(count("all"), 54);
        assert_eq!(count("all -layer:U"), 33);
        assert_eq!(count("face:U"), 9);
        assert_eq!(count("layer:F&layer:R"), 8);
        assert_eq!(count("touching:3"), 24);
        assert_eq!(count("touching:2"), 24);
        assert_eq!(count("touching:1"), 6);
        assert!(model.region("nonsense").is_none());
    }
}
