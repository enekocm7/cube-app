//! A puzzle in three dimensions, as a scramble leaves it, flattened to the
//! polygons a view paints.
//!
//! [`scramble::preview`](crate::scramble::preview) draws a flat net and keeps
//! no state, so this follows the scramble on the models the algorithm library
//! uses. A [`Solid`] is the scrambled puzzle; [`Solid::project`] turns it by
//! an [`Orientation`] and returns what faces the viewer, already sorted so
//! that each polygon is drawn over the ones before it. The view only fills
//! those polygons in order.
//!
//! Every event has a solid. Text that is not a scramble for the event has
//! none, and the caller falls back to the net.

use std::f64::consts::{PI, SQRT_2, TAU};
use std::sync::LazyLock;

use crate::event::WcaEvent;
use crate::library::case_origins;
use crate::library::geometry::{
    Vec3, add, centroid, cross, dot, normalized, rotated, same, scale, subtract,
};
use crate::library::model::{Model, Op, State, StickerModel, square1_corner};
use crate::theme::ThemeColor;
use crate::trainer::{AlgCase, AlgSet, sticker_fills};

/// How far the viewer stands from the middle of the puzzle, in times the
/// distance to its farthest corner. Closer exaggerates the perspective.
const CAMERA_DISTANCE: f64 = 5.;

/// How much of its place a sticker fills. The rest shows the plastic.
const STICKER_FILL: f64 = 0.9;

/// The plastic between the stickers, as dark as the lines of the net.
const BODY: ThemeColor = ThemeColor::new(0x10, 0x10, 0x10);

/// How far one key press turns the puzzle.
const KEY_STEP: f64 = PI / 12.;

/// How far dragging across the whole view turns the puzzle.
const DRAG_TURN: f64 = PI;

const fn hex(colour: u32) -> ThemeColor {
    ThemeColor::new((colour >> 16) as u8, (colour >> 8) as u8, colour as u8)
}

// The colours of each net, by the name `TNoodle` gives the face.
const CUBE_COLOURS: [(&str, ThemeColor); 6] = [
    ("U", hex(0xffffff)),
    ("R", hex(0xff0000)),
    ("F", hex(0x00ff00)),
    ("D", hex(0xffff00)),
    ("L", hex(0xff8000)),
    ("B", hex(0x0000ff)),
];
const PYRAMINX_COLOURS: [(&str, ThemeColor); 4] = [
    ("F", hex(0x00ff00)),
    ("D", hex(0xffff00)),
    ("L", hex(0xff0000)),
    ("R", hex(0x0000ff)),
];
const SKEWB_COLOURS: [(&str, ThemeColor); 6] = [
    ("U", hex(0xffffff)),
    ("R", hex(0x0000ff)),
    ("F", hex(0xff0000)),
    ("D", hex(0xffff00)),
    ("L", hex(0x00ff00)),
    ("B", hex(0xff8000)),
];
const FTO_COLOURS: [(&str, ThemeColor); 8] = [
    ("U", hex(0xffffff)),
    ("R", hex(0xff0000)),
    ("F", hex(0x00ff00)),
    ("L", hex(0x7c029e)),
    ("B", hex(0x0000ff)),
    ("BL", hex(0xff8000)),
    ("D", hex(0xffff00)),
    ("BR", hex(0x808080)),
];
const MEGAMINX_COLOURS: [(&str, ThemeColor); 12] = [
    ("U", hex(0xffffff)),
    ("BL", hex(0xffcc00)),
    ("BR", hex(0x0000b3)),
    ("R", hex(0xdd0000)),
    ("F", hex(0x006600)),
    ("L", hex(0x8a1aff)),
    ("D", hex(0x999999)),
    ("DR", hex(0xffffb3)),
    ("DBR", hex(0xff99ff)),
    ("B", hex(0x71e600)),
    ("DBL", hex(0xff8433)),
    ("DL", hex(0x88ddff)),
];

/// Square-1's colours, in the order `TNoodle` numbers its sides: left, back,
/// right, front, up and down.
const SQUARE1_COLOURS: [ThemeColor; 6] = [
    hex(0x0000ff),
    hex(0xff8000),
    hex(0x00ff00),
    hex(0xff0000),
    hex(0xffff00),
    hex(0xffffff),
];

/// The colours of one side of a Clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ClockSide {
    face: ThemeColor,
    dial: ThemeColor,
    /// The mark at twelve o'clock.
    top: ThemeColor,
    hand: ThemeColor,
    pin: ThemeColor,
}

const CLOCK_FRONT: ClockSide = ClockSide {
    face: hex(0x113366),
    dial: hex(0xccddee),
    top: hex(0xffcc44),
    hand: hex(0x113366),
    // The net draws each side's pins in the other side's pin colour.
    pin: hex(0x446699),
};
const CLOCK_BACK: ClockSide = ClockSide {
    face: hex(0xccddee),
    dial: hex(0x113366),
    top: hex(0xcc6600),
    hand: hex(0xccddee),
    pin: hex(0x88aacc),
};

/// A direction to turn the whole puzzle in, as the viewer sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spin {
    Left,
    Right,
    Up,
    Down,
    /// Around the line of sight, counter clockwise.
    RollLeft,
    /// Around the line of sight, clockwise.
    RollRight,
}

/// How a drag turns the puzzle, chosen when it begins, as in most 3D
/// viewers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grip {
    /// Turns it whichever way the pointer goes, like a ball under the hand.
    Free,
    /// Only around the vertical axis of the screen or only around the
    /// horizontal one, whichever the drag starts along.
    Straight,
    /// Around the line of sight, like a dial, following the pointer around
    /// the middle of the view.
    Roll,
}

/// The screen axis a [`Grip::Straight`] drag keeps to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lock {
    /// Only across: turns around the vertical axis.
    Across,
    /// Only up and down: tilts around the horizontal axis.
    Down,
}

/// A drag in progress on a view of the puzzle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drag {
    grip: Grip,
    lock: Option<Lock>,
}

impl Drag {
    pub const fn new(grip: Grip) -> Self {
        Self { grip, lock: None }
    }

    /// Turns `held` as the pointer moving from `from` to `to` asks. Both
    /// points are in times the size of the view, from its middle, with `x`
    /// to the right and `y` down.
    pub fn turn(&mut self, held: Orientation, from: [f64; 2], to: [f64; 2]) -> Orientation {
        let across = to[0] - from[0];
        let down = to[1] - from[1];
        match self.grip {
            Grip::Free => held.dragged(across, down),
            Grip::Straight => {
                // The first move decides, and the drag keeps to it.
                let lock = *self.lock.get_or_insert(if across.abs() >= down.abs() {
                    Lock::Across
                } else {
                    Lock::Down
                });
                match lock {
                    Lock::Across => held.dragged(across, 0.),
                    Lock::Down => held.dragged(0., down),
                }
            }
            Grip::Roll => {
                // Too near the middle, the angle of the pointer means nothing.
                let near = |point: [f64; 2]| point[0].hypot(point[1]) < 0.02;
                if near(from) || near(to) {
                    return held;
                }
                let angle = to[1].atan2(to[0]) - from[1].atan2(from[0]);
                // Kept within half a turn so crossing the left edge of the
                // middle never spins it the long way round. With `y` down,
                // a positive angle is clockwise, which is negative around
                // the line of sight.
                let angle = (angle + PI).rem_euclid(TAU) - PI;
                held.turned(2, -angle)
            }
        }
    }
}

/// How the puzzle is held in front of the viewer: any rotation at all.
///
/// Every turn is about an axis of the screen, never of the puzzle, so a drag
/// moves the side under the pointer the way the pointer goes however the
/// puzzle is held, and it can be turned over in any direction without limit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orientation {
    /// The rows of the matrix that turns a point of the puzzle to where the
    /// viewer sees it: `x` to the right, `y` up and `z` towards the viewer.
    rows: [Vec3; 3],
}

impl Orientation {
    /// The usual picture of a puzzle: the top, the front and the right side.
    pub const STANDARD: Self = Self {
        // Turned 35° to the right around the vertical axis, then tilted 25°
        // to show the top.
        rows: [
            [0.819_152_044_288_991_8, 0., -0.573_576_436_351_046],
            [
                -0.242_403_876_506_104,
                0.906_307_787_036_649_9,
                -0.346_188_613_058_754_15,
            ],
            [
                0.519_836_790_725_684_5,
                0.422_618_261_740_699_44,
                0.742_403_876_506_104_1,
            ],
        ],
    };

    /// Held turned `yaw` around the vertical axis, positive bringing the left
    /// side to the front, then tilted `pitch`, positive showing the top.
    #[cfg(test)]
    fn new(yaw: f64, pitch: f64) -> Self {
        Self {
            rows: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        }
        .turned(1, yaw)
        .turned(0, pitch)
    }

    /// Turned `angle` radians around the screen axis `axis` (0 is `x`, 1 is
    /// `y`, 2 is the line of sight), counter clockwise as seen from its tip.
    fn turned(self, axis: usize, angle: f64) -> Self {
        let mut unit = [0.; 3];
        unit[axis] = 1.;
        self.turned_about(unit, angle)
    }

    /// Turned `angle` radians around the unit vector `axis` of the screen,
    /// counter clockwise as seen from its tip.
    fn turned_about(self, axis: Vec3, angle: f64) -> Self {
        // The columns are where the puzzle's own axes point on the screen.
        let column = |index: usize| {
            let rows = &self.rows;
            rotated(
                [rows[0][index], rows[1][index], rows[2][index]],
                axis,
                angle,
            )
        };
        // Kept a pure rotation, so many small turns never skew the puzzle.
        let x = normalized(column(0));
        let z = normalized(cross(x, column(1)));
        let y = cross(z, x);
        Self {
            rows: [[x[0], y[0], z[0]], [x[1], y[1], z[1]], [x[2], y[2], z[2]]],
        }
    }

    /// Turns the puzzle one keyboard step.
    #[must_use]
    pub fn spun(self, spin: Spin) -> Self {
        match spin {
            Spin::Left => self.turned(1, -KEY_STEP),
            Spin::Right => self.turned(1, KEY_STEP),
            Spin::Up => self.turned(0, -KEY_STEP),
            Spin::Down => self.turned(0, KEY_STEP),
            Spin::RollLeft => self.turned(2, KEY_STEP),
            Spin::RollRight => self.turned(2, -KEY_STEP),
        }
    }

    /// Turns the puzzle with a drag of `across` to the right and `down`, both
    /// in times the size of the view, so the side under the pointer follows it.
    #[must_use]
    pub fn dragged(self, across: f64, down: f64) -> Self {
        let length = across.hypot(down);
        if length == 0. {
            return self;
        }
        // One turn around the axis square to the drag, so a diagonal drag
        // rolls the puzzle along it.
        self.turned_about([down / length, across / length, 0.], length * DRAG_TURN)
    }

    /// Whether this is [`Self::STANDARD`], and resetting would change nothing.
    pub fn is_standard(self) -> bool {
        self.rows
            .iter()
            .zip(Self::STANDARD.rows)
            .all(|(row, standard)| same(*row, standard))
    }

    fn rows(self) -> [Vec3; 3] {
        self.rows
    }
}

impl Default for Orientation {
    fn default() -> Self {
        Self::STANDARD
    }
}

/// A flat shape to fill, in a square that runs from `-1` to `1` both ways with
/// `x` to the right and `y` down.
#[derive(Clone, Debug, PartialEq)]
pub struct Polygon {
    pub points: Vec<[f32; 2]>,
    pub colour: ThemeColor,
}

/// A flat piece of a solid: plastic, a sticker, or a mark on a face.
#[derive(Clone, Debug)]
struct Patch {
    corners: Vec<Vec3>,
    /// Where its distance from the viewer is measured. What lies on a face
    /// shares the face's point, so the two are never drawn the wrong way round.
    anchor: Vec3,
    normal: Vec3,
    colour: ThemeColor,
    /// Patches of a lower rank are all drawn first.
    rank: u8,
    /// Whether it lies on the patch it shares its anchor with.
    over: bool,
}

/// Patches that hide one another in a known order: by rank, then from the
/// farthest to the nearest.
#[derive(Clone, Debug)]
struct Group {
    /// Groups are drawn from the farthest middle to the nearest.
    middle: Vec3,
    patches: Vec<Patch>,
}

impl Group {
    fn new(middle: Vec3) -> Self {
        Self {
            middle,
            patches: Vec::new(),
        }
    }

    /// Adds one flat patch, measured from its own middle.
    fn mark(&mut self, corners: Vec<Vec3>, normal: Vec3, rank: u8, colour: ThemeColor) {
        self.patches.push(Patch {
            anchor: centroid(&corners),
            corners,
            normal,
            colour,
            rank,
            over: false,
        });
    }

    /// Adds a face of plastic and, with a colour, the sticker on it.
    fn face(&mut self, corners: Vec<Vec3>, normal: Vec3, rank: u8, sticker: Option<ThemeColor>) {
        let anchor = centroid(&corners);
        if let Some(colour) = sticker {
            self.patches.push(Patch {
                corners: inset(&corners),
                anchor,
                normal,
                colour,
                rank,
                over: true,
            });
        }
        self.patches.push(Patch {
            corners,
            anchor,
            normal,
            colour: BODY,
            rank,
            over: false,
        });
    }
}

/// Shrinks a place to the sticker on it.
fn inset(corners: &[Vec3]) -> Vec<Vec3> {
    let middle = centroid(corners);
    corners
        .iter()
        .map(|&corner| add(middle, scale(subtract(corner, middle), STICKER_FILL)))
        .collect()
}

/// A scrambled puzzle in space: `x` to the right, `y` up and `z` towards a
/// viewer who has not turned it.
#[derive(Clone, Debug)]
pub struct Solid {
    groups: Vec<Group>,
    /// The distance from the middle to the farthest corner.
    reach: f64,
    net_height: u32,
}

/// The natural height of the net of `event`, in logical pixels.
const fn net_height(event: WcaEvent) -> u32 {
    match event {
        WcaEvent::Cube2x2 => 68,
        WcaEvent::Cube3x3 | WcaEvent::Cube3x3OneHanded | WcaEvent::Cube3x3Blind => 98,
        WcaEvent::Cube4x4 | WcaEvent::Cube4x4Blind => 128,
        WcaEvent::Cube5x5 | WcaEvent::Cube5x5Blind => 158,
        WcaEvent::Cube6x6 => 188,
        WcaEvent::Cube7x7 => 218,
        WcaEvent::Megaminx => 146,
        WcaEvent::Pyraminx => 170,
        WcaEvent::Skewb => 187,
        WcaEvent::Fto => 100,
        WcaEvent::Square1 => 244,
        WcaEvent::Clock => 150,
    }
}

/// Builds the puzzle as `scramble` leaves it, or `None` when the text is not
/// a scramble for `event`. The caller then draws the net.
pub fn solid(event: WcaEvent, scramble: &str) -> Option<Solid> {
    let groups = match Model::of(event) {
        Model::Stickers(_) => {
            let (model, state) = scrambled(event, scramble)?;
            vec![stickers(model, &state, &Shape::of(event))?]
        }
        model @ Model::Square1 => square1(&followed(model, scramble)?.0),
        model @ Model::Clock => {
            let (state, ops) = followed(model, scramble)?;
            let flips = ops.iter().filter(|op| matches!(op, Op::Flip)).count();
            vec![clock(&state, flips % 2 == 0)]
        }
    };
    Some(Solid::new(groups, net_height(event)))
}

impl Solid {
    fn new(groups: Vec<Group>, net_height: u32) -> Self {
        let reach = groups
            .iter()
            .flat_map(|group| &group.patches)
            .flat_map(|patch| &patch.corners)
            .map(|&corner| dot(corner, corner))
            .fold(0., f64::max)
            .sqrt();
        Self {
            groups,
            reach,
            net_height,
        }
    }
}

/// Builds a case of the library as its set draws it: held with the last
/// layer on top, in the colours of the set's drawings, with what says nothing
/// about the case left grey. `None` when its algorithm cannot be read.
pub fn case_solid(set: &AlgSet, case: &AlgCase) -> Option<Solid> {
    let state = case_origins(set, case)?;
    let groups = match set.model() {
        Model::Stickers(model) => {
            let fills = sticker_fills(model, set, &state)
                .into_iter()
                .map(ThemeColor::from_hex)
                .collect::<Option<Vec<_>>>()?;
            vec![painted(model, &fills, &Shape::of(set.event))]
        }
        Model::Square1 => square1(&state),
        Model::Clock => vec![clock(&state, true)],
    };
    Some(Solid::new(groups, net_height(set.event)))
}

/// Does `scramble` to the solved puzzle of `model`. `None` when it cannot be
/// read or cannot be done. No moves at all leave the puzzle solved, which is
/// still worth drawing.
fn followed(model: &Model, scramble: &str) -> Option<(State, Vec<Op>)> {
    let mut state = model.solved();
    if scramble.trim().is_empty() {
        return Some((state, Vec::new()));
    }
    let ops = model.parse(scramble).ok()?;
    model.apply(&mut state, &ops).then_some((state, ops))
}

/// The two turns a Megaminx scramble is made of, which the library's notation
/// has no name for: everything but one face turns a fifth, clockwise as seen
/// from the face opposite it. Each is which place every place takes its
/// sticker from, for `R+` and then `D+`.
static POCHMANN: LazyLock<[Vec<u16>; 2]> = LazyLock::new(|| {
    let Model::Stickers(model) = Model::of(WcaEvent::Megaminx) else {
        unreachable!("A Megaminx is made of stickers");
    };
    [("DBR", "L"), ("D", "U")].map(|(around, left)| {
        let axis = face_normal(model, around);
        // The layer left behind is what a turn of its face moves, and the
        // middle of that face.
        let behind = model.face_names.iter().position(|face| *face == left);
        let layer = model.base(left).map(|base| model.source(base));
        let stays = |index: usize, face: u8| {
            layer.is_some_and(|layer| usize::from(layer[index]) != index)
                || Some(usize::from(face)) == behind
        };
        let mut source: Vec<u16> = (0..model.stickers.len() as u16).collect();
        for (index, sticker) in model.stickers.iter().enumerate() {
            if stays(index, sticker.face) {
                continue;
            }
            let center = rotated(sticker.center, axis, -TAU / 5.);
            let normal = rotated(sticker.normal, axis, -TAU / 5.);
            let target = model
                .stickers
                .iter()
                .position(|other| same(other.center, center) && same(other.normal, normal))
                .expect("a fifth of a turn maps a Megaminx onto itself");
            source[target] = index as u16;
        }
        source
    })
});

/// Reads a word of a Megaminx scramble such as `R++` or `D-`: which of the
/// [`POCHMANN`] turns it is, and how many fifths.
fn pochmann(word: &str) -> Option<(usize, usize)> {
    let turn = match word.chars().next()? {
        'R' => 0,
        'D' => 1,
        _ => return None,
    };
    let fifths = match &word[1..] {
        "+" => 1,
        "++" => 2,
        "--" => 3,
        "-" => 4,
        _ => return None,
    };
    Some((turn, fifths))
}

/// Follows `scramble` on the sticker model of `event`: the model, and which
/// face's sticker sits at each of its places. `None` when the event is not
/// made of stickers or the text is not a scramble for it.
fn scrambled(event: WcaEvent, scramble: &str) -> Option<(&'static StickerModel, State)> {
    let model = Model::of(event);
    let Model::Stickers(stickers) = model else {
        return None;
    };
    if event != WcaEvent::Megaminx {
        return Some((stickers, followed(model, scramble)?.0));
    }
    let mut state = model.solved();
    for word in scramble.split_whitespace() {
        if let Some((turn, fifths)) = pochmann(word) {
            for _ in 0..fifths {
                state = POCHMANN[turn]
                    .iter()
                    .map(|&from| state[usize::from(from)])
                    .collect();
            }
        } else {
            let ops = model.parse(word).ok()?;
            model.apply(&mut state, &ops);
        }
    }
    Some((stickers, state))
}

/// How a puzzle of stickers is coloured and held.
struct Shape {
    colours: &'static [(&'static str, ThemeColor)],
    /// The face on top, or the one underneath with `-1`.
    up: (&'static str, f64),
    /// The face towards the viewer.
    front: &'static str,
}

impl Shape {
    fn of(event: WcaEvent) -> Self {
        let (colours, up): (&'static [(&'static str, ThemeColor)], _) = match event {
            // A Pyraminx has no face on top: it stands on `D`.
            WcaEvent::Pyraminx => (&PYRAMINX_COLOURS, ("D", -1.)),
            WcaEvent::Skewb => (&SKEWB_COLOURS, ("U", 1.)),
            WcaEvent::Fto => (&FTO_COLOURS, ("U", 1.)),
            WcaEvent::Megaminx => (&MEGAMINX_COLOURS, ("U", 1.)),
            _ => (&CUBE_COLOURS, ("U", 1.)),
        };
        Self {
            colours,
            up,
            front: "F",
        }
    }
}

/// The way the face called `name` faces.
fn face_normal(model: &StickerModel, name: &str) -> Vec3 {
    let face = model.face_names.iter().position(|face| *face == name);
    let sticker = model
        .stickers
        .iter()
        .find(|sticker| Some(usize::from(sticker.face)) == face);
    sticker.map_or([0., 1., 0.], |sticker| sticker.normal)
}

fn apply(rows: &[Vec3; 3], point: Vec3) -> Vec3 {
    [
        dot(rows[0], point),
        dot(rows[1], point),
        dot(rows[2], point),
    ]
}

/// Builds a puzzle of stickers: the faces of the solid under the stickers.
fn stickers(model: &StickerModel, state: &[u16], shape: &Shape) -> Option<Group> {
    let colours: Vec<ThemeColor> = model
        .face_names
        .iter()
        .map(|name| {
            let entry = shape.colours.iter().find(|(face, _)| face == name);
            entry.map(|&(_, colour)| colour)
        })
        .collect::<Option<_>>()?;
    let fills: Vec<ThemeColor> = state
        .iter()
        .map(|&face| colours[usize::from(face)])
        .collect();
    Some(painted(model, &fills, shape))
}

/// Builds a puzzle of stickers with `fills[place]` on each place.
fn painted(model: &StickerModel, fills: &[ThemeColor], shape: &Shape) -> Group {
    // The model is built wherever its moves are simplest to write. This turns
    // it to how the puzzle is held.
    let up = scale(face_normal(model, shape.up.0), shape.up.1);
    let front = face_normal(model, shape.front);
    let front = normalized(subtract(front, scale(up, dot(front, up))));
    let held = [cross(up, front), up, front];

    let mut group = Group::new([0.; 3]);
    let mut outlines: Vec<Vec<Vec3>> = vec![Vec::new(); model.face_names.len()];
    let mut normals = vec![[0.; 3]; model.face_names.len()];
    for (place, &fill) in model.stickers.iter().zip(fills) {
        let corners: Vec<Vec3> = place
            .polygon
            .iter()
            .map(|&corner| apply(&held, corner))
            .collect();
        let normal = apply(&held, place.normal);
        outlines[usize::from(place.face)].extend(&corners);
        normals[usize::from(place.face)] = normal;
        group.mark(inset(&corners), normal, 1, fill);
    }
    for (points, normal) in outlines.into_iter().zip(normals) {
        group.mark(outline(&points, normal), normal, 0, BODY);
    }
    group
}

/// The convex outline of `points`, which all lie on a plane facing `normal`.
fn outline(points: &[Vec3], normal: Vec3) -> Vec<Vec3> {
    let helper = if normal[0].abs() < 0.9 {
        [1., 0., 0.]
    } else {
        [0., 1., 0.]
    };
    let across = normalized(cross(normal, helper));
    let up = cross(normal, across);
    let mut flat: Vec<(f64, f64, Vec3)> = points
        .iter()
        .map(|&point| (dot(point, across), dot(point, up), point))
        .collect();
    flat.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let turn = |a: &(f64, f64, Vec3), b: &(f64, f64, Vec3), c: &(f64, f64, Vec3)| {
        (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
    };
    // Andrew's monotone chain: the lower hull, then the upper one.
    let mut hull: Vec<(f64, f64, Vec3)> = Vec::new();
    for pass in 0..2 {
        let start = hull.len();
        let ordered: Box<dyn Iterator<Item = &(f64, f64, Vec3)>> = if pass == 0 {
            Box::new(flat.iter())
        } else {
            Box::new(flat.iter().rev())
        };
        for point in ordered {
            while hull.len() >= start + 2
                && turn(&hull[hull.len() - 2], &hull[hull.len() - 1], point) <= 1e-9
            {
                hull.pop();
            }
            hull.push(*point);
        }
        // The last point of a pass opens the next one.
        hull.pop();
    }
    hull.into_iter().map(|(_, _, point)| point).collect()
}

/// The tangent of 15 degrees: how far from the middle of a side the cut
/// through a Square-1 meets it, on a puzzle two units wide.
const SQUARE1_CUT: f64 = 0.267_949_192_431_122_7;

/// The top and the bottom of a Square-1's middle layer.
const SQUARE1_MIDDLE: f64 = 1. / 3.;

/// The colour of a Square-1 piece's top or bottom, then of its sides in the
/// order they follow each other, clockwise as seen from outside that layer.
fn square1_colours(piece: u16) -> (ThemeColor, Vec<ThemeColor>) {
    let colours = &SQUARE1_COLOURS;
    let up = piece < 8;
    let cap = colours[if up { 4 } else { 5 }];
    if square1_corner(piece) {
        let place = usize::from(if up { piece } else { 15 - piece });
        let (first, second) = (colours[(place / 2 + 3) % 4], colours[place / 2]);
        // The bottom layer is the mirror image of the top one.
        let sides = if up {
            vec![first, second]
        } else {
            vec![second, first]
        };
        (cap, sides)
    } else {
        let place = usize::from(if up { piece } else { 14 - piece });
        (cap, vec![colours[place / 2]])
    }
}

/// A wall between `from` and `to`, two points at the same height on the rim
/// of a layer that runs down or up to `other`. It faces away from the middle.
fn wall(from: Vec3, to: Vec3, other: f64) -> (Vec<Vec3>, Vec3) {
    let along = subtract(to, from);
    let mut normal = normalized([along[2], 0., -along[0]]);
    if dot(normal, add(from, to)) < 0. {
        normal = scale(normal, -1.);
    }
    let corners = vec![from, to, [to[0], other, to[2]], [from[0], other, from[2]]];
    (corners, normal)
}

/// Builds a Square-1 from its 24 wedges and the state of its middle layer.
///
/// `TNoodle` numbers the wedges of each layer clockwise as seen from outside
/// that layer, starting at the cut, on the front. The cut runs from just left
/// of the middle of the front to just right of the middle of the back, and a
/// `/` turns everything to its right.
///
/// Each layer is a group. A layer never hides part of one nearer the viewer,
/// and within a layer the walls can only be hidden by its top or bottom.
fn square1(state: &[u16]) -> Vec<Group> {
    let rim = 1. / (PI / 12.).cos();
    let mut groups = Vec::with_capacity(3);
    for (slots, start, side) in [(&state[..12], 105., 1.), (&state[12..24], -105., -1.)] {
        let (outer, inner) = (side, side * SQUARE1_MIDDLE);
        let at = |degrees: f64, radius: f64, height: f64| -> Vec3 {
            let angle = f64::to_radians(degrees);
            [radius * angle.cos(), height, side * radius * angle.sin()]
        };
        let mut group = Group::new([0., f64::midpoint(outer, inner), 0.]);
        // A corner may lie across the cut, so the walk starts where a piece does.
        let first = (0..12)
            .find(|&slot| slots[slot] != slots[(slot + 11) % 12])
            .unwrap_or(0);
        let mut walked = 0;
        while walked < 12 {
            let slot = first + walked;
            let piece = slots[slot % 12];
            let from = start + 30. * slot as f64;
            let edge: Vec<(f64, f64)> = if square1_corner(piece) {
                vec![(from, rim), (from + 30., SQRT_2), (from + 60., rim)]
            } else {
                vec![(from, rim), (from + 30., rim)]
            };
            walked += edge.len() - 1;
            let (cap, sides) = square1_colours(piece);
            for (height, facing, sticker) in [(outer, side, Some(cap)), (inner, -side, None)] {
                let mut corners = vec![[0., height, 0.]];
                corners.extend(
                    edge.iter()
                        .map(|&(angle, radius)| at(angle, radius, height)),
                );
                group.face(corners, [0., facing, 0.], 1, sticker);
            }
            for (pair, colour) in edge.windows(2).zip(sides) {
                let (corners, normal) = wall(
                    at(pair[0].0, pair[0].1, outer),
                    at(pair[1].0, pair[1].1, outer),
                    inner,
                );
                group.face(corners, normal, 0, Some(colour));
            }
        }
        groups.push(group);
    }

    // The middle layer: the half left of the cut, which never moves, and the
    // half right of it, which a `/` turns over.
    let [left, back, right, front, ..] = SQUARE1_COLOURS;
    let (near, far) = ([-SQUARE1_CUT, 1.], [SQUARE1_CUT, -1.]);
    let halves = [
        (
            [near, [-1., 1.], [-1., -1.], far],
            [front, left, back],
            false,
        ),
        (
            [near, [1., 1.], [1., -1.], far],
            [front, right, back],
            state[24] != 0,
        ),
    ];
    let mut group = Group::new([0.; 3]);
    // Half a turn around the line through the middle, square to the cut.
    let (sine, cosine) = (PI / 12.).sin_cos();
    let axis = [cosine, 0., sine];
    let turned = |point: Vec3| subtract(scale(axis, 2. * dot(point, axis)), point);
    for (plan, colours, flipped) in halves {
        let place = |point: Vec3| if flipped { turned(point) } else { point };
        let mut add_face = |corners: Vec<Vec3>, normal: Vec3, rank, sticker| {
            let corners = corners.into_iter().map(place).collect();
            group.face(corners, place(normal), rank, sticker);
        };
        for (height, up) in [(SQUARE1_MIDDLE, 1.), (-SQUARE1_MIDDLE, -1.)] {
            let corners = plan.iter().map(|&[x, z]| [x, height, z]).collect();
            add_face(corners, [0., up, 0.], 1, None);
        }
        for (pair, colour) in plan.windows(2).zip(colours) {
            let (corners, normal) = wall(
                [pair[0][0], SQUARE1_MIDDLE, pair[0][1]],
                [pair[1][0], SQUARE1_MIDDLE, pair[1][1]],
                -SQUARE1_MIDDLE,
            );
            add_face(corners, normal, 0, Some(colour));
        }
    }
    groups.push(group);
    groups
}

// A Clock, in the units of its net.
const CLOCK_RADIUS: f64 = 70.;
/// The distance between two dials, and from the middle to a corner's bulge.
const CLOCK_GRID: f64 = 42.;
const CLOCK_BULGE: f64 = 21.;
const CLOCK_DIAL: f64 = 14.;
/// How far from the middle of its dial each hour mark is.
const CLOCK_MARKS: f64 = 17.5;
const CLOCK_HAND: f64 = 10.;
const CLOCK_HUB: f64 = 2.;
const CLOCK_PIN: f64 = 4.;
/// Half the thickness of the puzzle.
const CLOCK_DEPTH: f64 = 9.;

/// A circle on a side of a Clock, as a polygon in that side's own flat
/// coordinates: `x` to the right and `y` down, for someone looking at it.
fn disc(middle: [f64; 2], radius: f64, sides: u32) -> Vec<[f64; 2]> {
    (0..sides)
        .map(|side| {
            let angle = TAU * f64::from(side) / f64::from(sides);
            [
                middle[0] + radius * angle.cos(),
                middle[1] + radius * angle.sin(),
            ]
        })
        .collect()
}

/// The rim of a Clock: a circle with a bulge around each corner dial.
fn clock_rim() -> Vec<[f64; 2]> {
    (0..72)
        .map(|step| {
            let angle = TAU * f64::from(step) / 72.;
            let (sine, cosine) = angle.sin_cos();
            let mut radius = CLOCK_RADIUS;
            for corner in [[1., 1.], [1., -1.], [-1., 1.], [-1., -1.]] {
                let middle = scale([corner[0], corner[1], 0.], CLOCK_GRID);
                let along = middle[0] * cosine + middle[1] * sine;
                let off = middle[0] * sine - middle[1] * cosine;
                let reached = CLOCK_BULGE * CLOCK_BULGE - off * off;
                if reached >= 0. {
                    radius = radius.max(along + reached.sqrt());
                }
            }
            [radius * cosine, radius * sine]
        })
        .collect()
}

/// Builds a Clock from its 18 dials: the nine facing the viewer, then the
/// nine on the other side as someone who walked around the puzzle sees them.
/// `front_up` says the side facing the viewer is the front, which it is until
/// a scramble turns the puzzle over.
///
/// A corner's two dials sit on one axle, so their hands always point the same
/// way in space.
fn clock(state: &[u16], front_up: bool) -> Group {
    let mut group = Group::new([0.; 3]);
    let rim = clock_rim();
    for (index, &[x, y]) in rim.iter().enumerate() {
        let [next_x, next_y] = rim[(index + 1) % rim.len()];
        let (corners, normal) = wall_between([x, y], [next_x, next_y]);
        group.mark(corners, normal, 0, BODY);
    }
    for (side, dials) in state.chunks(9).take(2).enumerate() {
        let facing = side == 0;
        let colours = if facing == front_up {
            CLOCK_FRONT
        } else {
            CLOCK_BACK
        };
        // The other side is seen from behind, where right is left.
        let turn = if facing { 1. } else { -1. };
        let normal = [0., 0., turn];
        let lift = |points: Vec<[f64; 2]>| -> Vec<Vec3> {
            points
                .into_iter()
                .map(|[x, y]| [turn * x, -y, turn * CLOCK_DEPTH])
                .collect()
        };
        group.mark(lift(rim.clone()), normal, 1, colours.face);
        for (dial, &hour) in dials.iter().enumerate() {
            let middle = [
                (dial % 3) as f64 * CLOCK_GRID - CLOCK_GRID,
                (dial / 3) as f64 * CLOCK_GRID - CLOCK_GRID,
            ];
            group.mark(lift(disc(middle, CLOCK_DIAL, 20)), normal, 2, colours.dial);
            // A point `along` the hand of a given hour and `across` it.
            let towards = |hour: f64, along: f64, across: f64| {
                let (sine, cosine) = (hour * TAU / 12.).sin_cos();
                [
                    middle[0] + along * sine + across * cosine,
                    middle[1] - along * cosine + across * sine,
                ]
            };
            for mark in 0..12 {
                let (radius, rank, colour) = if mark == 0 {
                    (2., 4, colours.top)
                } else {
                    (1., 3, colours.dial)
                };
                let at = towards(f64::from(mark), CLOCK_MARKS, 0.);
                group.mark(lift(disc(at, radius, 6)), normal, rank, colour);
            }
            let hour = f64::from(hour);
            let hand = vec![
                towards(hour, 0., -CLOCK_HUB),
                towards(hour, CLOCK_HAND, 0.),
                towards(hour, 0., CLOCK_HUB),
            ];
            group.mark(lift(hand), normal, 5, colours.hand);
            group.mark(lift(disc(middle, CLOCK_HUB, 8)), normal, 5, colours.hand);
        }
        for pin in [[-1., -1.], [1., -1.], [-1., 1.], [1., 1.]] {
            let middle = [pin[0] * CLOCK_BULGE, pin[1] * CLOCK_BULGE];
            group.mark(lift(disc(middle, CLOCK_PIN, 10)), normal, 6, colours.pin);
        }
    }
    group
}

/// The part of a Clock's edge between two points of its rim.
fn wall_between(from: [f64; 2], to: [f64; 2]) -> (Vec<Vec3>, Vec3) {
    let corners = vec![
        [from[0], from[1], CLOCK_DEPTH],
        [to[0], to[1], CLOCK_DEPTH],
        [to[0], to[1], -CLOCK_DEPTH],
        [from[0], from[1], -CLOCK_DEPTH],
    ];
    let mut normal = normalized([to[1] - from[1], from[0] - to[0], 0.]);
    if dot(normal, [from[0] + to[0], from[1] + to[1], 0.]) < 0. {
        normal = scale(normal, -1.);
    }
    (corners, normal)
}

impl Solid {
    /// The natural height of the net of the same puzzle, in logical pixels,
    /// so that both drawings can be given the same room.
    pub const fn net_height(&self) -> u32 {
        self.net_height
    }

    /// The patches of `group` that face a viewer at `eye` once turned by
    /// `rows`, in the order to draw them: by rank, then from the farthest to
    /// the nearest, and a sticker right after the plastic under it.
    fn facing<'a>(group: &'a Group, rows: &[Vec3; 3], eye: Vec3) -> Vec<&'a Patch> {
        let mut seen: Vec<(f64, &Patch)> = group
            .patches
            .iter()
            .filter_map(|patch| {
                let towards = subtract(eye, apply(rows, patch.anchor));
                (dot(apply(rows, patch.normal), towards) > 0.)
                    .then(|| (dot(towards, towards), patch))
            })
            .collect();
        seen.sort_by(|a, b| {
            (a.1.rank.cmp(&b.1.rank))
                .then(b.0.total_cmp(&a.0))
                .then(a.1.over.cmp(&b.1.over))
        });
        seen.into_iter().map(|(_, patch)| patch).collect()
    }

    /// Returns what the viewer sees of the puzzle held as `orientation` says:
    /// the polygons to fill, each drawn over the ones before it.
    ///
    /// Only what faces the viewer is returned. The drawing keeps its size as
    /// the puzzle turns.
    pub fn project(&self, orientation: Orientation) -> Vec<Polygon> {
        let rows = orientation.rows();
        let camera = self.reach * CAMERA_DISTANCE;
        let eye = [0., 0., camera];
        // The farthest corner reaches exactly the edge of the square when it
        // is seen side on, so nothing is ever cut off.
        let fit = (camera * camera - self.reach * self.reach).sqrt() / (self.reach * camera);
        let mut groups: Vec<(f64, &Group)> = self
            .groups
            .iter()
            .map(|group| {
                let towards = subtract(eye, apply(&rows, group.middle));
                (dot(towards, towards), group)
            })
            .collect();
        groups.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut polygons = Vec::new();
        for (_, group) in groups {
            polygons.extend(Self::facing(group, &rows, eye).into_iter().map(|patch| {
                Polygon {
                    points: patch
                        .corners
                        .iter()
                        .map(|&corner| {
                            let [x, y, z] = apply(&rows, corner);
                            let near = camera / (camera - z) * fit;
                            [(x * near) as f32, (-y * near) as f32]
                        })
                        .collect(),
                    colour: patch.colour,
                }
            }));
        }
        polygons
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use tnoodle::puzzle::{
        ClockPuzzle, CubePuzzle, Face, FtoPuzzle, MegaminxPuzzle, PyraminxPuzzle, SkewbPuzzle,
        SquareOnePuzzle,
    };
    use tnoodle::{Puzzle, PuzzleState};

    use super::*;
    use crate::library::geometry::same;
    use crate::scramble;

    const CUBES: [(WcaEvent, usize); 10] = [
        (WcaEvent::Cube2x2, 2),
        (WcaEvent::Cube3x3, 3),
        (WcaEvent::Cube3x3OneHanded, 3),
        (WcaEvent::Cube3x3Blind, 3),
        (WcaEvent::Cube4x4, 4),
        (WcaEvent::Cube4x4Blind, 4),
        (WcaEvent::Cube5x5, 5),
        (WcaEvent::Cube5x5Blind, 5),
        (WcaEvent::Cube6x6, 6),
        (WcaEvent::Cube7x7, 7),
    ];

    /// Each face of the net: its `TNoodle` face, the way it faces, and the
    /// directions its columns and rows run in.
    const NET: [(Face, Vec3, Vec3, Vec3); 6] = [
        (Face::U, [0., 1., 0.], [1., 0., 0.], [0., 0., 1.]),
        (Face::R, [1., 0., 0.], [0., 0., -1.], [0., -1., 0.]),
        (Face::F, [0., 0., 1.], [1., 0., 0.], [0., -1., 0.]),
        (Face::D, [0., -1., 0.], [1., 0., 0.], [0., 0., -1.]),
        (Face::L, [-1., 0., 0.], [0., 0., 1.], [0., -1., 0.]),
        (Face::B, [0., 0., -1.], [-1., 0., 0.], [0., -1., 0.]),
    ];

    /// Checks every sticker of the solid against the state `TNoodle` draws
    /// its net from.
    fn assert_matches_the_net(event: WcaEvent, size: usize, scramble_text: &str) {
        let (model, state) = scrambled(event, scramble_text)
            .unwrap_or_else(|| panic!("{event:?} has no solid for {scramble_text}"));
        let net = CubePuzzle::new(size)
            .solved_state()
            .apply_algorithm(scramble_text)
            .unwrap();
        let step = 2. / size as f64;
        for (face, normal, right, down) in NET {
            for row in 0..size {
                for column in 0..size {
                    let across = -1. + step * (column as f64 + 0.5);
                    let below = -1. + step * (row as f64 + 0.5);
                    let at = add(normal, add(scale(right, across), scale(down, below)));
                    let place = model
                        .stickers
                        .iter()
                        .position(|sticker| same(sticker.center, at))
                        .unwrap();
                    let expected = Face::ALL[usize::from(net.sticker(face, row, column))].name();
                    assert_eq!(
                        model.face_names[usize::from(state[place])],
                        expected,
                        "{event:?} {face:?} row {row} column {column} after {scramble_text}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_state_matches_the_net_for_every_cube() {
        for (event, size) in CUBES {
            for _ in 0..3 {
                assert_matches_the_net(event, size, &scramble::generate(event));
            }
        }
    }

    #[test]
    fn the_state_follows_wide_moves_and_rotations() {
        assert_matches_the_net(WcaEvent::Cube3x3Blind, 3, "R U2 F' L D2 B Rw' Uw2");
        assert_matches_the_net(WcaEvent::Cube4x4Blind, 4, "Rw U Fw2 D' Uw B2 L x y'");
        assert_matches_the_net(WcaEvent::Cube5x5Blind, 5, "Bw L Dw2 R' 3Rw 3Uw'");
        assert_matches_the_net(WcaEvent::Cube6x6, 6, "3Rw 3Uw' 3Fw2 Bw Lw' Dw2 R U F");
        assert_matches_the_net(WcaEvent::Cube7x7, 7, "3Bw' 3Lw2 3Dw 3Rw' Uw Fw2 L D B'");
    }

    /// A sequence of `count` words of `alphabet`, the same for the same seed.
    fn random_words(alphabet: &[&str], count: usize, seed: u64) -> String {
        let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let words: Vec<&str> = (0..count)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                alphabet[(state >> 33) as usize % alphabet.len()]
            })
            .collect();
        words.join(" ")
    }

    /// The fill of every shape of a net, in the order they are drawn.
    fn fills(svg: &str) -> Vec<String> {
        svg.split("fill=\"#")
            .skip(1)
            .map(|rest| rest[..6].to_owned())
            .collect()
    }

    /// Checks a puzzle of stickers against its net without knowing where the
    /// net draws each sticker: over many scrambles, every place of the solid
    /// must show the same run of colours as one sticker of the net, and every
    /// sticker of the net must be claimed by one place.
    ///
    /// No two stickers share a run, so this pairs each place with one sticker,
    /// and the pairing carries every move of the net to the same move of the
    /// solid. Faces are coloured by name on both sides, so a solid that passes
    /// is the puzzle the net shows.
    fn assert_follows_the_net(event: WcaEvent, alphabet: &[&str], outlines: usize) {
        let shape = Shape::of(event);
        let mut net: Vec<String> = Vec::new();
        let mut ours: Vec<String> = Vec::new();
        for seed in 0..24 {
            let text = random_words(alphabet, 40, seed);
            let drawn = scramble::preview(event, &text)
                .unwrap_or_else(|| panic!("{event:?} has no net for {text}"));
            // Some nets draw a white outline before the stickers of a face.
            let shapes = fills(&drawn.svg);
            let per_face = shapes.len() / shape.colours.len();
            let stickers = shapes
                .iter()
                .enumerate()
                .filter(|(index, _)| index % per_face >= outlines)
                .map(|(_, fill)| fill);
            let (model, state) = scrambled(event, &text).unwrap();
            net.resize(model.stickers.len(), String::new());
            ours.resize(model.stickers.len(), String::new());
            assert_eq!(stickers.clone().count(), model.stickers.len(), "{event:?}");
            for (run, fill) in net.iter_mut().zip(stickers) {
                run.push_str(fill);
            }
            for (run, &face) in ours.iter_mut().zip(&state) {
                let name = model.face_names[usize::from(face)];
                let colour = shape.colours.iter().find(|entry| entry.0 == name).unwrap();
                write!(run, "{:06x}", colour.1.rgb()).unwrap();
            }
        }
        net.sort();
        ours.sort();
        let mut distinct = net.clone();
        distinct.dedup();
        assert_eq!(distinct.len(), net.len(), "{event:?} needs more scrambles");
        assert_eq!(ours, net, "{event:?}");
    }

    #[test]
    fn the_pyraminx_follows_its_net() {
        let moves = [
            "U", "U'", "L", "L'", "R", "R'", "B", "B'", "u", "u'", "l", "l'", "r", "r'", "b", "b'",
        ];
        assert_follows_the_net(WcaEvent::Pyraminx, &moves, 0);
    }

    #[test]
    fn the_skewb_follows_its_net() {
        let moves = ["R", "R'", "U", "U'", "L", "L'", "B", "B'"];
        assert_follows_the_net(WcaEvent::Skewb, &moves, 0);
    }

    #[test]
    fn the_megaminx_follows_its_net() {
        let moves = [
            "R++", "R--", "D++", "D--", "R+", "R-", "D+", "D-", "U", "U'", "F", "L2", "BR2'",
            "DBL'", "B", "DR2",
        ];
        assert_follows_the_net(WcaEvent::Megaminx, &moves, 0);
        // A real scramble has line breaks between its rows.
        let text = scramble::generate(WcaEvent::Megaminx);
        assert!(text.contains('\n') && solid(WcaEvent::Megaminx, &text).is_some());
    }

    #[test]
    fn the_fto_follows_its_net() {
        let moves = [
            "U", "R", "F", "L", "B", "BL", "D", "BR", "U'", "R'", "F'", "L'", "B'", "BL'", "D'",
            "BR'",
        ];
        assert_follows_the_net(WcaEvent::Fto, &moves, 1);
    }

    #[test]
    fn the_colours_are_those_of_the_nets() {
        fn check(ours: &[(&str, ThemeColor)], net: &[(&str, tnoodle::svg::Color)]) {
            assert_eq!(ours.len(), net.len());
            for (face, colour) in ours {
                let &(_, net) = net.iter().find(|(name, _)| name == face).unwrap();
                assert_eq!(
                    *colour,
                    ThemeColor::new(net.red(), net.green(), net.blue()),
                    "{face}"
                );
            }
        }
        check(
            &CUBE_COLOURS,
            CubePuzzle::new(3).default_color_scheme_entries(),
        );
        check(
            &PYRAMINX_COLOURS,
            PyraminxPuzzle::new().default_color_scheme_entries(),
        );
        check(
            &SKEWB_COLOURS,
            SkewbPuzzle::new().default_color_scheme_entries(),
        );
        check(
            &FTO_COLOURS,
            FtoPuzzle::new().default_color_scheme_entries(),
        );
        check(
            &MEGAMINX_COLOURS,
            MegaminxPuzzle::new().default_color_scheme_entries(),
        );
        let [left, back, right, front, up, down] = SQUARE1_COLOURS;
        check(
            &[
                ("L", left),
                ("B", back),
                ("R", right),
                ("F", front),
                ("U", up),
                ("D", down),
            ],
            SquareOnePuzzle::new().default_color_scheme_entries(),
        );
        let (front, back) = (CLOCK_FRONT, CLOCK_BACK);
        check(
            &[
                ("Front", front.face),
                ("FrontClock", front.dial),
                ("FrontTopClock", front.top),
                ("FrontHand", front.hand),
                ("FrontHandBorder", front.hand),
                // Each side's pins take the other side's pin colour.
                ("FrontPin", back.pin),
                ("Back", back.face),
                ("BackClock", back.dial),
                ("BackTopClock", back.top),
                ("BackHand", back.hand),
                ("BackHandBorder", back.hand),
                ("BackPin", front.pin),
            ],
            ClockPuzzle::new().default_color_scheme_entries(),
        );
    }

    /// The stickers and marks of a solid, as a colour and rounded corners in
    /// a fixed order, to compare two solids whatever order they were built in.
    fn painted(solid: &Solid, moved: impl Fn(&Patch, Vec3) -> Vec3) -> Vec<String> {
        let mut all: Vec<String> = solid
            .groups
            .iter()
            .flat_map(|group| &group.patches)
            .filter(|patch| patch.colour != BODY)
            .map(|patch| {
                let mut corners: Vec<String> = patch
                    .corners
                    .iter()
                    .map(|&corner| {
                        let [x, y, z] = moved(patch, corner);
                        // Adding zero turns a rounded `-0` into `0`.
                        let round = |value: f64| (value * 1e4).round() / 1e4 + 0.;
                        format!("{:.4},{:.4},{:.4}", round(x), round(y), round(z))
                    })
                    .collect();
                corners.sort();
                format!("{:06x} {}", patch.colour.rgb(), corners.join(" "))
            })
            .collect();
        all.sort();
        all
    }

    const SQUARE1_SCRAMBLE: &str = "(-2,0) / (5,2) / (6,-3) / (3,0) / (4,-2) / (5,0) / (0,-3) / (-3,0) / (-3,-1) / (2,-5) / (0,-2) / (-4,0)";

    #[test]
    fn the_square1_state_is_the_one_the_net_is_drawn_from() {
        for _ in 0..5 {
            let text = scramble::generate(WcaEvent::Square1);
            let (state, _) = followed(Model::of(WcaEvent::Square1), &text).unwrap();
            let net = SquareOnePuzzle::new()
                .solved_state()
                .apply_algorithm(&text)
                .unwrap();
            let pieces: Vec<u16> = net.pieces().iter().map(|&piece| u16::from(piece)).collect();
            assert_eq!(&state[..24], &pieces[..], "{text}");
            assert_eq!(state[24] == 0, net.is_slice_solved(), "{text}");
            assert!(solid(WcaEvent::Square1, &text).is_some());
        }
    }

    #[test]
    fn a_solved_square1_is_a_cube_with_one_colour_a_side() {
        let cube = solid(WcaEvent::Square1, "").unwrap();
        let [left, back, right, front, up, down] = SQUARE1_COLOURS;
        let sides = [
            (left, [-1., 0., 0.]),
            (back, [0., 0., -1.]),
            (right, [1., 0., 0.]),
            (front, [0., 0., 1.]),
            (up, [0., 1., 0.]),
            (down, [0., -1., 0.]),
        ];
        let stickers: Vec<&Patch> = cube
            .groups
            .iter()
            .flat_map(|group| &group.patches)
            .filter(|patch| patch.colour != BODY)
            .collect();
        // Sixteen on top and bottom, and around the sides twelve on each
        // layer and six in the middle.
        assert_eq!(stickers.len(), 16 + 12 + 12 + 6);
        for patch in stickers {
            let (_, normal) = sides.iter().find(|side| side.0 == patch.colour).unwrap();
            assert!(same(patch.normal, *normal), "{patch:?}");
            // Every sticker lies flat on the surface of the cube.
            for &corner in &patch.corners {
                assert!((dot(corner, *normal) - 1.).abs() < 1e-9, "{patch:?}");
            }
        }
        assert!((cube.reach - 3_f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn turning_a_square1_layer_turns_its_pieces() {
        let slashed = solid(WcaEvent::Square1, &format!("{SQUARE1_SCRAMBLE} /")).unwrap();
        let after = solid(WcaEvent::Square1, &format!("{SQUARE1_SCRAMBLE} / (2,-3)")).unwrap();
        // The top turns clockwise as seen from above, and the bottom as seen
        // from below, a twelfth for each step.
        let turned = painted(&slashed, |patch, point| {
            let twelfths = if patch.anchor[1] > 0.4 {
                -2.
            } else if patch.anchor[1] < -0.4 {
                -3.
            } else {
                0.
            };
            rotated(point, [0., 1., 0.], twelfths * TAU / 12.)
        });
        assert_eq!(painted(&after, |_, point| point), turned);
        assert_ne!(painted(&slashed, |_, point| point), turned);
    }

    #[test]
    fn a_slash_turns_the_right_half_of_a_square1_over() {
        let before = solid(WcaEvent::Square1, SQUARE1_SCRAMBLE).unwrap();
        let after = solid(WcaEvent::Square1, &format!("{SQUARE1_SCRAMBLE} /")).unwrap();
        let (sine, cosine) = (PI / 12.).sin_cos();
        let axis = [cosine, 0., sine];
        let turned = painted(&before, |patch, point| {
            // What lies right of the cut goes half a turn around the line
            // square to it.
            let inward = subtract(patch.anchor, scale(patch.normal, 0.05));
            if dot(inward, axis) > 0. {
                subtract(scale(axis, 2. * dot(point, axis)), point)
            } else {
                point
            }
        });
        assert_eq!(painted(&after, |_, point| point), turned);
        assert_ne!(painted(&before, |_, point| point), turned);
    }

    #[test]
    fn a_square1_that_cannot_be_slashed_has_no_solid() {
        assert!(solid(WcaEvent::Square1, "(1,0) /").is_some());
        assert!(solid(WcaEvent::Square1, "(2,0) /").is_none());
        // A corner lying across the cut is still drawn once.
        let across = solid(WcaEvent::Square1, "(2,0)").unwrap();
        let stickers = across
            .groups
            .iter()
            .flat_map(|group| &group.patches)
            .filter(|patch| patch.colour != BODY);
        assert_eq!(stickers.count(), 16 + 12 + 12 + 6);
    }

    #[test]
    fn the_clock_state_is_the_one_the_net_is_drawn_from() {
        for _ in 0..5 {
            let text = scramble::generate(WcaEvent::Clock);
            let (state, _) = followed(Model::of(WcaEvent::Clock), &text).unwrap();
            let net = ClockPuzzle::new()
                .solved_state()
                .apply_algorithm(&text)
                .unwrap();
            let dials: Vec<u16> = net.positions().iter().map(|&dial| dial as u16).collect();
            assert_eq!(state, dials, "{text}");
            assert!(solid(WcaEvent::Clock, &text).is_some());
        }
    }

    /// Where each hand of a Clock points, with the middle of its dial: the
    /// hands are the only triangles.
    fn hands(clock: &Solid) -> Vec<(Vec3, Vec3)> {
        let patches = &clock.groups[0].patches;
        patches
            .iter()
            .filter(|patch| patch.corners.len() == 3)
            .map(|hand| {
                let base = scale(add(hand.corners[0], hand.corners[2]), 0.5);
                (base, normalized(subtract(hand.corners[1], base)))
            })
            .collect()
    }

    #[test]
    fn a_solved_clock_points_every_hand_up() {
        let clock = solid(WcaEvent::Clock, "").unwrap();
        let hands = hands(&clock);
        assert_eq!(hands.len(), 18);
        assert!(hands.iter().all(|hand| same(hand.1, [0., 1., 0.])));
    }

    #[test]
    fn clock_hands_turn_clockwise_and_corners_share_an_axle() {
        // Every dial of the front goes to three o'clock, which is to the
        // right. The corners of the back go to nine, which from behind is the
        // same way in space, as they are the other ends of the front's.
        let clock = solid(WcaEvent::Clock, "ALL3+").unwrap();
        let hands_after = hands(&clock);
        let front: Vec<_> = hands_after.iter().filter(|hand| hand.0[2] > 0.).collect();
        let back: Vec<_> = hands_after.iter().filter(|hand| hand.0[2] < 0.).collect();
        assert_eq!((front.len(), back.len()), (9, 9));
        assert!(front.iter().all(|hand| same(hand.1, [1., 0., 0.])));
        for hand in back {
            let corner = hand.0[0].abs() > 1. && hand.0[1].abs() > 1.;
            let expected = if corner { [1., 0., 0.] } else { [0., 1., 0.] };
            assert!(same(hand.1, expected), "{hand:?}");
        }
        // One pin turns the corner it is next to on both sides: `UR` is the
        // upper right of the front and so the upper left from behind.
        let clock = solid(WcaEvent::Clock, "UR3+").unwrap();
        for (base, towards) in hands(&clock) {
            let turned = if base[2] > 0. {
                base[0] > -1. && base[1] > -1.
            } else {
                base[0] > 1. && base[1] > 1.
            };
            let expected = if turned { [1., 0., 0.] } else { [0., 1., 0.] };
            assert!(same(towards, expected), "{base:?}");
        }
    }

    #[test]
    fn turning_a_clock_over_shows_its_back() {
        let face = |text: &str| {
            let clock = solid(WcaEvent::Clock, text).unwrap();
            let shown = clock.project(Orientation::new(0., 0.));
            // The face is the first thing drawn after the edge.
            let face = shown.iter().find(|polygon| polygon.colour != BODY);
            face.unwrap().colour
        };
        assert_eq!(face("UR2+"), CLOCK_FRONT.face);
        assert_eq!(face("UR2+ y2 U1-"), CLOCK_BACK.face);
        assert_eq!(face("y2 y2"), CLOCK_FRONT.face);
    }

    #[test]
    fn every_event_has_a_solid_for_its_scrambles() {
        for event in WcaEvent::ALL {
            let text = scramble::generate(event);
            let made = solid(event, &text).unwrap_or_else(|| panic!("{event:?}: {text}"));
            assert!(!made.project(Orientation::STANDARD).is_empty(), "{event:?}");
            assert!(solid(event, "").is_some(), "{event:?}");
        }
    }

    #[test]
    fn every_puzzle_stays_inside_its_square_and_fills_it() {
        for event in WcaEvent::ALL {
            let made = solid(event, &scramble::generate(event)).unwrap();
            let mut held = Orientation::STANDARD;
            let mut widest: f32 = 0.;
            for step in 0..120 {
                held = held.dragged(0.043, if step % 30 < 15 { 0.037 } else { -0.037 });
                for polygon in made.project(held) {
                    for [x, y] in polygon.points {
                        assert!(x.abs() <= 1.0001 && y.abs() <= 1.0001, "{event:?}");
                        widest = widest.max(x.abs()).max(y.abs());
                    }
                }
            }
            assert!(widest > 0.8, "{event:?} {widest}");
        }
    }

    #[test]
    fn puzzles_are_held_the_way_their_scrambles_are_written() {
        // Seen from the front and a little above, each shows its front face,
        // and its top face where it has one.
        let above = Orientation::new(0., 25. * PI / 180.);
        let shows = |event: WcaEvent, colours: &[(&str, ThemeColor)], face: &str| {
            let colour = colours.iter().find(|entry| entry.0 == face).unwrap().1;
            let shown = solid(event, "").unwrap().project(above);
            shown.iter().any(|polygon| polygon.colour == colour)
        };
        assert!(shows(WcaEvent::Pyraminx, &PYRAMINX_COLOURS, "F"));
        assert!(!shows(WcaEvent::Pyraminx, &PYRAMINX_COLOURS, "D"));
        for face in ["U", "F"] {
            assert!(shows(WcaEvent::Skewb, &SKEWB_COLOURS, face));
            assert!(shows(WcaEvent::Fto, &FTO_COLOURS, face));
            assert!(shows(WcaEvent::Megaminx, &MEGAMINX_COLOURS, face));
        }
        for face in ["D", "B"] {
            assert!(!shows(WcaEvent::Skewb, &SKEWB_COLOURS, face));
            assert!(!shows(WcaEvent::Fto, &FTO_COLOURS, face));
            assert!(!shows(WcaEvent::Megaminx, &MEGAMINX_COLOURS, face));
        }
        // A puzzle's right side is on the viewer's right once it is turned
        // into view.
        let right_of_middle = |event: WcaEvent, colours: &[(&str, ThemeColor)], face: &str| {
            let colour = colours.iter().find(|entry| entry.0 == face).unwrap().1;
            let turned = Orientation::new(-50. * PI / 180., 25. * PI / 180.);
            let drawn = solid(event, "").unwrap().project(turned);
            let at: Vec<f32> = drawn
                .iter()
                .filter(|polygon| polygon.colour == colour)
                .flat_map(|polygon| polygon.points.iter().map(|point| point[0]))
                .collect();
            assert!(!at.is_empty(), "{event:?} {face}");
            at.iter().sum::<f32>() / at.len() as f32 > 0.
        };
        assert!(right_of_middle(WcaEvent::Pyraminx, &PYRAMINX_COLOURS, "R"));
        assert!(right_of_middle(WcaEvent::Skewb, &SKEWB_COLOURS, "R"));
        assert!(right_of_middle(WcaEvent::Fto, &FTO_COLOURS, "R"));
        assert!(right_of_middle(WcaEvent::Megaminx, &MEGAMINX_COLOURS, "R"));
    }

    #[test]
    fn plastic_is_drawn_before_what_lies_on_it() {
        let held = Orientation::STANDARD.dragged(0.3, -0.2);
        // A cube draws all its plastic first, so its stickers share paths.
        let cube = solid(WcaEvent::Cube5x5, "").unwrap();
        let shown = cube.project(held);
        let first_sticker = shown.iter().position(|polygon| polygon.colour != BODY);
        let first_sticker = first_sticker.unwrap();
        assert!(first_sticker > 0);
        let stickers = &shown[first_sticker..];
        assert!(stickers.iter().all(|polygon| polygon.colour != BODY));
        // Within a rank the farthest comes first, and a sticker follows the
        // plastic it lies on.
        for event in [WcaEvent::Cube5x5, WcaEvent::Square1, WcaEvent::Clock] {
            let made = solid(event, &scramble::generate(event)).unwrap();
            let rows = held.rows();
            let eye = [0., 0., made.reach * CAMERA_DISTANCE];
            for group in &made.groups {
                let seen = Solid::facing(group, &rows, eye);
                assert!(!seen.is_empty(), "{event:?}");
                let distance = |patch: &Patch| {
                    let towards = subtract(eye, apply(&rows, patch.anchor));
                    dot(towards, towards)
                };
                for pair in seen.windows(2) {
                    assert!(pair[0].rank <= pair[1].rank, "{event:?}");
                    if pair[0].rank == pair[1].rank {
                        assert!(distance(pair[0]) >= distance(pair[1]), "{event:?}");
                    }
                    if pair[1].over {
                        assert!(same(pair[0].anchor, pair[1].anchor), "{event:?}");
                        assert!(!pair[0].over && pair[0].colour == BODY, "{event:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_solid_asks_for_the_room_of_the_net() {
        for event in WcaEvent::ALL {
            let net = scramble::preview(event, "").unwrap();
            let made = solid(event, "").unwrap();
            assert_eq!(made.net_height(), net.height, "{event:?}");
        }
    }

    #[test]
    fn text_that_is_not_a_scramble_has_no_solid() {
        assert!(solid(WcaEvent::Cube3x3, "R U R' U'").is_some());
        assert!(solid(WcaEvent::Cube3x3, "R++ D--").is_none());
        assert!(solid(WcaEvent::Cube3x3, "nonsense").is_none());
        assert!(solid(WcaEvent::Cube2x2, "Rw").is_none());
        assert!(solid(WcaEvent::Megaminx, "R++ Q").is_none());
        assert!(solid(WcaEvent::Clock, "UR3").is_none());
    }

    #[test]
    fn the_standard_view_shows_the_top_the_front_and_the_right() {
        for (event, size) in CUBES {
            let shown = solid(event, "").unwrap().project(Orientation::STANDARD);
            let (plastic, stickers) = shown.split_at(3);
            assert!(plastic.iter().all(|polygon| polygon.colour == BODY));
            assert!(plastic.iter().all(|polygon| polygon.points.len() == 4));
            assert_eq!(stickers.len(), 3 * size * size, "{event:?}");
            let count = |name: &str| {
                let colour = CUBE_COLOURS.iter().find(|entry| entry.0 == name).unwrap().1;
                stickers
                    .iter()
                    .filter(|polygon| polygon.colour == colour)
                    .count()
            };
            for face in ["U", "F", "R"] {
                assert_eq!(count(face), size * size, "{event:?} {face}");
            }
        }
    }

    #[test]
    fn a_face_is_drawn_the_right_way_round() {
        // Seen from the front with no tilt, the front face fills the middle:
        // its right side is the cube's right side and its top is the top.
        let cube = solid(WcaEvent::Cube3x3, "R").unwrap();
        let front = Orientation::new(0., 0.);
        let shown = cube.project(front);
        let white = CUBE_COLOURS[0].1;
        let yellow = CUBE_COLOURS[3].1;
        let green = CUBE_COLOURS[2].1;
        let middle = |polygon: &Polygon| {
            let count = polygon.points.len() as f32;
            let sum = polygon.points.iter().fold([0., 0.], |sum, point| {
                [sum[0] + point[0], sum[1] + point[1]]
            });
            [sum[0] / count, sum[1] / count]
        };
        // `R` brings the bottom's right column to the front, so yellow
        // arrives on the right, in every row, and green keeps the rest.
        let arrived: Vec<[f32; 2]> = shown
            .iter()
            .filter(|polygon| polygon.colour == yellow)
            .map(middle)
            .collect();
        assert_eq!(arrived.len(), 3);
        assert!(arrived.iter().all(|at| at[0] > 0.3));
        let kept = shown
            .iter()
            .filter(|polygon| polygon.colour == green)
            .map(middle);
        assert!(kept.clone().count() == 6 && kept.clone().all(|at| at[0] < 0.3));
        assert!(shown.iter().all(|polygon| polygon.colour != white));
    }

    #[test]
    fn keys_turn_the_puzzle_and_opposite_keys_undo_each_other() {
        let start = Orientation::STANDARD;
        let close =
            |a: Orientation, b: Orientation| (0..3).all(|row| same(a.rows[row], b.rows[row]));
        assert_ne!(start.spun(Spin::Left), start);
        assert!(close(start.spun(Spin::Left).spun(Spin::Right), start));
        assert!(close(start.spun(Spin::Up).spun(Spin::Down), start));
        // Twenty-four steps go all the way round.
        let mut held = start;
        for _ in 0..24 {
            held = held.spun(Spin::Right);
        }
        assert!(close(held, start));
    }

    #[test]
    fn turning_right_shows_the_left_side_and_up_shows_the_bottom() {
        let cube = solid(WcaEvent::Cube3x3, "").unwrap();
        let has = |held: Orientation, name: &str| {
            let colour = CUBE_COLOURS.iter().find(|entry| entry.0 == name).unwrap().1;
            cube.project(held)
                .iter()
                .any(|polygon| polygon.colour == colour)
        };
        let front = Orientation::new(0., 0.);
        assert!(has(front.spun(Spin::Right), "L") && !has(front.spun(Spin::Right), "R"));
        assert!(has(front.spun(Spin::Left), "R") && !has(front.spun(Spin::Left), "L"));
        assert!(has(front.spun(Spin::Up), "D") && !has(front.spun(Spin::Up), "U"));
        assert!(has(front.spun(Spin::Down), "U") && !has(front.spun(Spin::Down), "D"));
        // A drag moves the side under the pointer the same way.
        assert!(has(front.dragged(0.1, 0.), "L"));
        assert!(has(front.dragged(0., 0.1), "U"));
    }

    #[test]
    fn the_puzzle_turns_over_freely_in_every_direction() {
        let front = Orientation::new(0., 0.);
        let facing = |held: Orientation| {
            // The face whose middle sticker is the nearest to the viewer.
            let rows = held.rows();
            let towards = |name: &str| match name {
                "U" => [0., 1., 0.],
                "D" => [0., -1., 0.],
                "R" => [1., 0., 0.],
                "L" => [-1., 0., 0.],
                "F" => [0., 0., 1.],
                _ => [0., 0., -1.],
            };
            ["U", "D", "R", "L", "F", "B"]
                .into_iter()
                .max_by(|a, b| apply(&rows, towards(a))[2].total_cmp(&apply(&rows, towards(b))[2]))
                .unwrap()
        };
        assert_eq!(facing(front), "F");
        // Tilting keeps going past the top, round to the back and the bottom.
        let mut held = front;
        let mut seen = Vec::new();
        for _ in 0..24 {
            held = held.spun(Spin::Down);
            seen.push(facing(held));
        }
        for face in ["U", "B", "D", "F"] {
            assert!(seen.contains(&face), "{face}");
        }
        // Upside down, a drag to the right still moves the front to the
        // right, and a drag down still moves it down.
        let mut upside_down = front;
        for _ in 0..12 {
            upside_down = upside_down.spun(Spin::Down);
        }
        for held in [front, upside_down, Orientation::STANDARD.dragged(0.7, -1.3)] {
            let column = |held: Orientation, index: usize| {
                let rows = held.rows();
                [rows[0][index], rows[1][index], rows[2][index]]
            };
            // The puzzle axis nearest to the viewer, followed on the screen.
            let index = (0..3)
                .max_by(|&a, &b| {
                    column(held, a)[2]
                        .abs()
                        .total_cmp(&column(held, b)[2].abs())
                })
                .unwrap();
            let sign = column(held, index)[2].signum();
            let near = |held: Orientation| scale(column(held, index), sign);
            let before = near(held);
            assert!(near(held.dragged(0.05, 0.))[0] > before[0]);
            assert!(near(held.dragged(0., 0.05))[1] < before[1]);
        }
    }

    /// Where the puzzle's axis `index` points on the screen.
    fn axis_on_screen(held: Orientation, index: usize) -> Vec3 {
        let rows = held.rows();
        [rows[0][index], rows[1][index], rows[2][index]]
    }

    #[test]
    fn rolling_turns_the_puzzle_around_the_line_of_sight() {
        let front = Orientation::new(0., 0.);
        // Counter clockwise brings the top to the left.
        let up = axis_on_screen(front.spun(Spin::RollLeft), 1);
        assert!(up[0] < 0. && up[2].abs() < 1e-9);
        let up = axis_on_screen(front.spun(Spin::RollRight), 1);
        assert!(up[0] > 0. && up[2].abs() < 1e-9);
        let back = front.spun(Spin::RollLeft).spun(Spin::RollRight);
        assert!((0..3).all(|row| same(back.rows[row], front.rows[row])));
        // Dragging clockwise around the middle, from the right to the
        // bottom, turns the top clockwise to the right by as much.
        let mut drag = Drag::new(Grip::Roll);
        let rolled = drag.turn(front, [0.4, 0.], [0., 0.4]);
        assert!(same(axis_on_screen(rolled, 1), [1., 0., 0.]));
        // The line of sight never moves.
        let held = Orientation::STANDARD;
        let rolled = drag.turn(held, [0.3, -0.2], [-0.1, 0.35]);
        let sight = |held: Orientation| [held.rows[2][0], held.rows[2][1], held.rows[2][2]];
        assert!(same(sight(rolled), sight(held)));
        // Crossing the left edge goes the short way.
        let rolled = drag.turn(front, [-0.4, 0.01], [-0.4, -0.01]);
        assert!(axis_on_screen(rolled, 1)[1] > 0.99);
        // A pointer on the very middle turns nothing.
        assert_eq!(drag.turn(held, [0., 0.], [0.3, 0.3]), held);
    }

    #[test]
    fn a_straight_drag_keeps_to_the_axis_it_starts_along() {
        let held = Orientation::STANDARD;
        let mut across = Drag::new(Grip::Straight);
        let mut turned = across.turn(held, [0., 0.], [0.1, 0.02]);
        turned = across.turn(turned, [0.1, 0.02], [0.12, 0.3]);
        // Only turned around the vertical axis of the screen: everything
        // keeps its height.
        for index in 0..3 {
            assert!(
                (axis_on_screen(turned, index)[1] - axis_on_screen(held, index)[1]).abs() < 1e-9
            );
        }
        assert!(!turned.is_standard());
        let mut down = Drag::new(Grip::Straight);
        let mut tilted = down.turn(held, [0., 0.], [0.01, 0.1]);
        tilted = down.turn(tilted, [0.01, 0.1], [0.4, 0.1]);
        for index in 0..3 {
            assert!(
                (axis_on_screen(tilted, index)[0] - axis_on_screen(held, index)[0]).abs() < 1e-9
            );
        }
        // A free drag follows both.
        let free = Drag::new(Grip::Free).turn(held, [0., 0.], [0.1, 0.1]);
        assert_eq!(free, held.dragged(0.1, 0.1));
    }

    #[test]
    fn every_library_case_has_a_solid_in_the_colours_of_its_drawing() {
        let views = [
            Orientation::STANDARD,
            Orientation::STANDARD.spun(Spin::Down).spun(Spin::Down),
            Orientation::new(PI, -0.4),
        ];
        for set in crate::library::library_sets() {
            for case in set.cases {
                let made =
                    case_solid(set, case).unwrap_or_else(|| panic!("{} {}", set.id, case.name));
                let Model::Stickers(model) = set.model() else {
                    assert!(
                        !made.project(Orientation::STANDARD).is_empty(),
                        "{}",
                        case.name
                    );
                    continue;
                };
                // The colours the set's drawing gives each place, grey ones
                // included.
                let origins = case_origins(set, case).unwrap();
                let fills: Vec<ThemeColor> = sticker_fills(model, set, &origins)
                    .into_iter()
                    .map(|fill| ThemeColor::from_hex(fill).unwrap())
                    .collect();
                for held in views {
                    for polygon in made.project(held) {
                        assert!(
                            polygon.colour == BODY || fills.contains(&polygon.colour),
                            "{} {}",
                            set.id,
                            case.name
                        );
                    }
                }
                // Every place is drawn with its own fill.
                let group = &made.groups[0];
                let stickers: Vec<ThemeColor> = group
                    .patches
                    .iter()
                    .filter(|patch| patch.rank == 1)
                    .map(|patch| patch.colour)
                    .collect();
                assert_eq!(stickers, fills, "{} {}", set.id, case.name);
            }
        }
    }

    #[test]
    fn a_case_is_held_with_its_last_layer_on_top() {
        let top = ThemeColor::from_hex("#f5d90a").unwrap();
        let blank = ThemeColor::from_hex("#7e868c").unwrap();
        let from_above = Orientation::new(0., std::f64::consts::FRAC_PI_2);
        let colours = |set: &AlgSet, name: &str| {
            let case = set.cases.iter().find(|case| case.name == name).unwrap();
            let shown = case_solid(set, case).unwrap().project(from_above);
            shown
                .into_iter()
                .map(|polygon| polygon.colour)
                .filter(|&colour| colour != BODY)
                .collect::<Vec<_>>()
        };
        // A PLL has its whole top in the colour of the last layer.
        let pll = &crate::trainer::PLL;
        let seen = colours(pll, pll.cases[0].name);
        assert_eq!(seen.len(), 9);
        assert!(seen.iter().all(|&colour| colour == top));
        // An OLL only says which stickers face up: the rest are grey.
        let oll = &crate::trainer::OLL;
        let case = oll.cases[0].name;
        let seen = colours(oll, case);
        assert!(seen.contains(&blank) && seen.contains(&top), "{case}");
    }

    #[test]
    fn the_standard_orientation_turns_right_then_tilts() {
        let made = Orientation::new(-35. * PI / 180., 25. * PI / 180.);
        for (row, standard) in made.rows.iter().zip(Orientation::STANDARD.rows) {
            for (value, expected) in row.iter().zip(standard) {
                assert!((value - expected).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn many_small_drags_keep_the_puzzle_square() {
        let mut held = Orientation::STANDARD;
        for step in 0..10_000 {
            held = held.dragged(0.013, if step % 7 < 4 { 0.021 } else { -0.017 });
        }
        let rows = held.rows();
        for a in 0..3 {
            assert!((dot(rows[a], rows[a]) - 1.).abs() < 1e-9);
            for b in a + 1..3 {
                assert!(dot(rows[a], rows[b]).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn only_the_standard_orientation_is_standard() {
        assert!(Orientation::default().is_standard());
        assert!(!Orientation::STANDARD.spun(Spin::Left).is_standard());
    }
}
