//! A small drawing of a case, for any puzzle: a cube's top with the sides
//! around it, a solid from the front and the back, a Square-1's two layers or
//! a Clock's two faces. Stickers are colored by where they began, so that a
//! set can leave out what says nothing about its cases.

use std::fmt::Write as _;

use super::{AlgCase, AlgSet};
use crate::event::WcaEvent;
use crate::library::case_origins;
use crate::library::geometry::{Vec3, add, cross, dot, normalized, scale, subtract};
use crate::library::model::{Model, StickerModel, square1_corner};

/// How wide and tall the drawing is, in logical pixels.
const VIEW: u32 = 100;

/// How a drawing is drawn, in the units of the view.
#[derive(Clone, Copy)]
struct Style {
    /// How many pixels wide the SVG is rasterized.
    pixels: u32,
    /// Space around a sticker.
    gap: f32,
    /// How deep the strips showing the sides are.
    strip: f32,
    /// How round a sticker's corners are.
    radius: f32,
    /// How wide the lines between pieces are.
    outline: f32,
    /// Whether to show what is out of sight: a solid's far side and the back
    /// of a Clock.
    far_side: bool,
}

/// A drawing shown large, sharp on high-density screens.
const FULL: Style = Style {
    pixels: 300,
    gap: 1.5,
    strip: 11.,
    radius: 1.5,
    outline: 0.5,
    far_side: true,
};

/// A drawing in a list. It is rasterized near the size it is shown at, since
/// shrinking one a lot blurs it, and its lines are heavier to survive being
/// small. What can't be made out at this size is left to the full drawing.
const THUMBNAIL: Style = Style {
    pixels: 128,
    gap: 3.4,
    strip: 12.,
    radius: 3.,
    outline: 1.5,
    far_side: false,
};

/// The most of a sticker its gap takes up, so that small stickers survive.
const MOST_GAP: f32 = 0.16;

/// Cases are drawn the way they are solved: with the last layer on top, and
/// green at the front.
const TOP: &str = "#f5d90a";
const FRONT: &str = "#30a46c";
const RIGHT: &str = "#f76b15";
const BACK: &str = "#0091ff";
const LEFT: &str = "#e5484d";
const DOWN: &str = "#f1f3f5";

/// A sticker that says nothing about the case.
const BLANK: &str = "#7e868c";

/// What separates pieces drawn side by side.
const OUTLINE: &str = "#1a1d1e";

/// Where a solid is seen from, up and to the right of its front.
const CAMERA: Vec3 = [0.42, 0.72, 0.9];

/// How much of the view the solid's second, far side takes up.
const FAR_SIDE: f32 = 0.42;

/// The color of a face, by its name in the puzzle's notation.
fn face_color(name: &str) -> &'static str {
    match name {
        "U" => TOP,
        "F" => FRONT,
        "R" => RIGHT,
        "B" => BACK,
        "L" => LEFT,
        "D" => DOWN,
        "BL" | "DL" => "#8e4ec6",
        "BR" | "DR" => "#e93d82",
        "DBL" => "#70e000",
        "DBR" => "#12a594",
        _ => "#ad7f58",
    }
}

fn rectangle(svg: &mut String, [x, y, width, height]: [f32; 4], fill: &str, gap: f32, radius: f32) {
    let _ = write!(
        svg,
        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{radius}" fill="{fill}"/>"#,
        x + gap / 2.,
        y + gap / 2.,
        width - gap,
        height - gap
    );
}

fn polygon(svg: &mut String, points: &[[f32; 2]], fill: &str, outline: f32) {
    let mut list = String::new();
    for [x, y] in points {
        let _ = write!(list, "{x:.2},{y:.2} ");
    }
    let _ = write!(
        svg,
        r#"<polygon points="{}" fill="{fill}" stroke="{OUTLINE}" stroke-width="{outline}" stroke-linejoin="round"/>"#,
        list.trim_end()
    );
}

/// What each place shows in a case of a puzzle of stickers.
pub(crate) fn sticker_fills(
    model: &StickerModel,
    set: &AlgSet,
    origins: &[u16],
) -> Vec<&'static str> {
    let shown = &set.show().places;
    origins
        .iter()
        .map(|&origin| {
            let sticker = &model.stickers[origin as usize];
            if shown[origin as usize] {
                face_color(model.face_names[usize::from(sticker.face)])
            } else {
                BLANK
            }
        })
        .collect()
}

/// Which of `size` rows or columns a coordinate from -1 to 1 falls in.
fn cell_of(coordinate: f64, size: usize) -> usize {
    let cell = f64::midpoint(coordinate, 1.) * size as f64;
    (cell as usize).min(size - 1)
}

/// Draws a cube from above, with the top row of each side around it.
fn draw_top(svg: &mut String, model: &StickerModel, fills: &[&str], style: Style) {
    let on_face = model.stickers.iter().filter(|sticker| sticker.face == 0);
    let size = (on_face.count() as f64).sqrt().round() as usize;
    let view = VIEW as f32;
    let strip = style.strip;
    let cell = (view - 2. * strip) / size as f32;
    let gap = style.gap.min(cell * MOST_GAP);
    let radius = style.radius.min(cell * MOST_GAP);
    let place = |index: usize| index as f32 * cell + strip;
    let top_row = 1. - 2. / size as f64;
    for (sticker, fill) in model.stickers.iter().zip(fills) {
        let [x, y, z] = sticker.center;
        let (column, row) = (cell_of(x, size), cell_of(z, size));
        let area = match model.face_names[usize::from(sticker.face)] {
            "U" => [place(column), place(row), cell, cell],
            "F" if y > top_row => [place(column), view - strip, cell, strip],
            "B" if y > top_row => [place(column), 0., cell, strip],
            "L" if y > top_row => [0., place(row), strip, cell],
            "R" if y > top_row => [view - strip, place(row), strip, cell],
            _ => continue,
        };
        rectangle(svg, area, fill, gap, radius);
    }
}

/// How much of its true depth a side sticker keeps when it is folded out
/// from the top, so that the top stays the larger part of the drawing.
const FOLDED_DEPTH: f64 = 0.45;

/// Where a sticker of the top layer lands when the sides are folded out flat
/// around the top, as `[x, z]` with the front toward `z`.
fn folded_out(point: Vec3, normal: Vec3) -> [f64; 2] {
    let up = [0., 1., 0.];
    let level = dot(normal, up);
    if level > 0.999 {
        return [point[0], point[2]];
    }
    // The side meets the top along a line: fold the side over it, down
    // its slope and out from the middle.
    let outward = normalized([normal[0], 0., normal[2]]);
    let across = (1. - level) / (1. - level * level).sqrt();
    let hinge = add(scale(outward, across), up);
    let along = cross(outward, up);
    let downhill = normalized(cross(along, normal));
    let downhill = if downhill[1] > 0. {
        scale(downhill, -1.)
    } else {
        downhill
    };
    let offset = subtract(point, hinge);
    let depth = dot(offset, downhill).max(0.) * FOLDED_DEPTH;
    let flat = add(
        scale(outward, across + depth),
        scale(along, dot(offset, along)),
    );
    [flat[0], flat[2]]
}

/// Draws a puzzle whose top is not a grid, such as a Megaminx, from above:
/// the top face with the stickers of its layer folded out around it.
fn draw_folded_top(
    svg: &mut String,
    model: &StickerModel,
    fills: &[&str],
    layer: &[bool],
    style: Style,
) {
    let shown: Vec<(Vec<[f64; 2]>, &str)> = model
        .stickers
        .iter()
        .zip(fills)
        .zip(layer)
        .filter(|&(_, &in_layer)| in_layer)
        .map(|((sticker, &fill), _)| {
            let inset = sticker.polygon.iter().map(|&point| {
                let toward = subtract(sticker.center, point);
                folded_out(add(point, scale(toward, 0.06)), sticker.normal)
            });
            (inset.collect(), fill)
        })
        .collect();
    let reach = shown
        .iter()
        .flat_map(|(points, _)| points)
        .map(|[x, z]| x.abs().max(z.abs()))
        .fold(0., f64::max);
    if reach <= 0. {
        return;
    }
    let view = f64::from(VIEW);
    let scale = view / 2. / reach * 0.97;
    for (points, fill) in &shown {
        let flat: Vec<[f32; 2]> = points
            .iter()
            .map(|[x, z]| {
                [
                    (view / 2. + x * scale) as f32,
                    (view / 2. + z * scale) as f32,
                ]
            })
            .collect();
        polygon(svg, &flat, fill, style.outline);
    }
}

/// Projects the half of a solid that faces `camera` into a square.
fn draw_side(
    svg: &mut String,
    model: &StickerModel,
    fills: &[&str],
    camera: Vec3,
    [left, top, size]: [f32; 3],
    outline: f32,
) {
    let camera = normalized(camera);
    let right = normalized(cross([0., 1., 0.], camera));
    let up = cross(camera, right);
    let far = model.stickers.iter().flat_map(|sticker| &sticker.polygon);
    let reach = far
        .map(|point| dot(*point, *point))
        .fold(0., f64::max)
        .sqrt();
    let project = |point: Vec3| {
        let scale = f64::from(size) / 2. / reach;
        let x = dot(point, right) * scale;
        let y = dot(point, up) * scale;
        [left + size / 2. + x as f32, top + size / 2. - y as f32]
    };
    for (sticker, fill) in model.stickers.iter().zip(fills) {
        if dot(sticker.normal, camera) <= 0.02 {
            continue;
        }
        let inset = sticker.polygon.iter().map(|&point| {
            let toward = subtract(sticker.center, point);
            project([
                point[0] + toward[0] * 0.06,
                point[1] + toward[1] * 0.06,
                point[2] + toward[2] * 0.06,
            ])
        });
        polygon(svg, &inset.collect::<Vec<_>>(), fill, outline);
    }
}

/// Draws a solid from the front and, smaller, in the corner, from behind.
fn draw_solid(svg: &mut String, model: &StickerModel, fills: &[&str], style: Style) {
    let view = VIEW as f32;
    if !style.far_side {
        draw_side(svg, model, fills, CAMERA, [0., 0., view], style.outline);
        return;
    }
    let near = view * (1. - FAR_SIDE * 0.55);
    draw_side(svg, model, fills, CAMERA, [0., 0., near], style.outline);
    let far = view * FAR_SIDE;
    let behind = CAMERA.map(|axis| -axis);
    let corner = [view - far, view - far, far];
    draw_side(svg, model, fills, behind, corner, style.outline);
}

/// How much further from the center a Square-1 corner's tip is than the ends
/// of its sides: `sqrt(2) * cos(15 degrees)`, so that a layer of corners and
/// edges in cube shape has straight sides.
const SQUARE1_TIP: f32 = 1.366;

/// Where a Square-1 piece's top ends, as a share of the piece.
const SQUARE1_TOP: f32 = 0.72;

/// A point of a Square-1 layer, at a slot's edge and a share of the radius.
fn slot_point(center: [f32; 2], radius: f32, slot: f32) -> [f32; 2] {
    let angle = (slot * 30. - 105.).to_radians();
    [
        center[0] + radius * angle.cos(),
        center[1] + radius * angle.sin(),
    ]
}

/// Returns the top of a Square-1 piece and the strip of its side around it.
/// A piece that spans two slots is a corner, with a tip in the middle.
fn piece_outlines(
    center: [f32; 2],
    radius: f32,
    from: f32,
    to: f32,
) -> (Vec<[f32; 2]>, Vec<[f32; 2]>) {
    let line = |share: f32| -> Vec<[f32; 2]> {
        let mut points = vec![slot_point(center, radius * share, from)];
        if to - from > 1. {
            let tip = radius * share * SQUARE1_TIP;
            points.push(slot_point(center, tip, f32::midpoint(from, to)));
        }
        points.push(slot_point(center, radius * share, to));
        points
    };
    let top_line = line(SQUARE1_TOP);
    let mut top = vec![center];
    top.extend(&top_line);
    // Out along one line and back along the other, or it would cross itself.
    let mut side = line(1.);
    side.extend(top_line.iter().rev());
    (top, side)
}

/// Draws one layer of a Square-1, its pieces as wedges around the center.
fn draw_layer(svg: &mut String, slots: &[u16], center: [f32; 2], radius: f32, outline: f32) {
    const SIDES: [&str; 4] = [FRONT, RIGHT, BACK, LEFT];
    // A corner may start at the last slot and go around to the first.
    let first = (0..12).find(|&slot| slots[slot] != slots[(slot + 11) % 12]);
    let first = first.unwrap_or(0);
    let mut slot = first;
    while slot < first + 12 {
        let piece = slots[slot % 12];
        let span = if square1_corner(piece) { 2 } else { 1 };
        let (from, to) = (slot as f32, (slot + span) as f32);
        let top = if piece < 8 { TOP } else { DOWN };
        let side = SIDES[usize::from(piece % 8) / 2];
        let (inner, outer) = piece_outlines(center, radius, from, to);
        polygon(svg, &outer, side, outline);
        polygon(svg, &inner, top, outline);
        slot += span;
    }
}

/// Draws a Square-1's layers and whether its middle is flipped.
fn draw_square1(svg: &mut String, state: &[u16], style: Style) {
    let view = VIEW as f32;
    let (radius, outline) = (view * 0.205, style.outline);
    draw_layer(svg, &state[..12], [view * 0.3, view * 0.3], radius, outline);
    draw_layer(
        svg,
        &state[12..24],
        [view * 0.7, view * 0.7],
        radius,
        outline,
    );
    let flipped = state[24] != 0;
    let (gap, round) = (style.gap, style.radius);
    rectangle(svg, [4., view - 14., 16., 10.], FRONT, gap, round);
    let second = if flipped { BACK } else { FRONT };
    rectangle(svg, [20., view - 14., 8., 10.], second, gap, round);
}

/// The measures of a Clock, as in the scramble preview: a face is 150 wide.
const CLOCK_FACE: f32 = 150.;
const CLOCK_RADIUS: f32 = 70.;
const DIAL_RADIUS: f32 = 14.;
const DIAL_SPACING: f32 = 42.;
const TICK_RADIUS: f32 = 17.5;
const PIN_SPACING: f32 = 21.;

/// The colors of one side of a Clock.
struct ClockColors {
    body: &'static str,
    dial: &'static str,
    noon: &'static str,
    pin: &'static str,
}

/// The front is dark with light dials, the back the other way around.
const CLOCK_FRONT: ClockColors = ClockColors {
    body: "#113366",
    dial: "#ccddee",
    noon: "#ffcc44",
    pin: "#446699",
};
const CLOCK_BACK: ClockColors = ClockColors {
    body: "#ccddee",
    dial: "#113366",
    noon: "#cc6600",
    pin: "#88aacc",
};

fn circle(svg: &mut String, [x, y]: [f32; 2], radius: f32, fill: &str, stroke: f32) {
    let _ = write!(
        svg,
        r#"<circle cx="{x}" cy="{y}" r="{radius}" fill="{fill}" stroke="{OUTLINE}" stroke-width="{stroke}"/>"#
    );
}

/// Draws one dial: its ticks, its face and its hand.
fn draw_dial(svg: &mut String, hour: u16, [x, y]: [f32; 2], colors: &ClockColors) {
    for tick in 0..12 {
        let angle = (tick as f32 * 30.).to_radians();
        let point = [x + TICK_RADIUS * angle.sin(), y - TICK_RADIUS * angle.cos()];
        let (fill, radius) = if tick == 0 {
            (colors.noon, 2.)
        } else {
            (colors.dial, 1.)
        };
        circle(svg, point, radius, fill, 0.);
    }
    circle(svg, [x, y], DIAL_RADIUS, colors.dial, 1.);
    let turn = u32::from(hour) * 30;
    let body = colors.body;
    let _ = write!(
        svg,
        r#"<polygon points="-2,0 0,-10 2,0 0,2" transform="translate({x} {y}) rotate({turn})" fill="{body}" stroke="{body}" stroke-width="1" stroke-linejoin="round"/>"#
    );
}

/// Draws one side of a Clock like the scramble preview does, its middle at
/// `[x, y]` and `scale` times the measures above.
fn draw_clock_face(svg: &mut String, dials: &[u16], [x, y, scale]: [f32; 3], colors: &ClockColors) {
    let _ = write!(svg, r#"<g transform="translate({x} {y}) scale({scale})">"#);
    let corners = [-DIAL_SPACING, DIAL_SPACING];
    let corners = corners.map(|x| corners.map(|y| [x, y]));
    for &corner in corners.as_flattened() {
        circle(svg, corner, PIN_SPACING, colors.body, 2.);
    }
    circle(svg, [0., 0.], CLOCK_RADIUS, colors.body, 2.);
    // Covers the outline where the corners meet the body.
    for &corner in corners.as_flattened() {
        circle(svg, corner, PIN_SPACING - 1., colors.body, 0.);
    }
    for (index, &hour) in dials.iter().enumerate() {
        let column = (index % 3) as f32 - 1.;
        let row = (index / 3) as f32 - 1.;
        let middle = [column * DIAL_SPACING, row * DIAL_SPACING];
        draw_dial(svg, hour, middle, colors);
    }
    for pin in [[-1., -1.], [1., -1.], [-1., 1.], [1., 1.]] {
        let place = [pin[0] * PIN_SPACING, pin[1] * PIN_SPACING];
        circle(svg, place, 4., colors.pin, 0.);
    }
    svg.push_str("</g>");
}

/// Draws a Clock's two faces side by side, or just its front.
fn draw_clock(svg: &mut String, state: &[u16], style: Style) {
    let view = VIEW as f32;
    let (middle, scale) = (view / 2., view / CLOCK_FACE);
    draw_clock_face(svg, &state[..9], [middle, middle, scale], &CLOCK_FRONT);
    if style.far_side {
        let back = [view + middle, middle, scale];
        draw_clock_face(svg, &state[9..], back, &CLOCK_BACK);
    }
}

/// How many times wider than tall [`case_diagram`] draws the cases of `set`: a
/// Clock's two faces go side by side, as in the scramble preview.
#[must_use]
pub fn diagram_ratio(set: &AlgSet) -> u32 {
    if matches!(set.model(), Model::Clock) {
        2
    } else {
        1
    }
}

/// Draws `case` as an SVG document, or `None` when its algorithm cannot be
/// read, which the tests rule out for the built-in sets.
pub fn case_diagram(set: &AlgSet, case: &AlgCase) -> Option<String> {
    draw(set, case, FULL)
}

/// Draws `case` for a list, where it is shown small.
pub fn case_thumbnail(set: &AlgSet, case: &AlgCase) -> Option<String> {
    draw(set, case, THUMBNAIL)
}

fn draw(set: &AlgSet, case: &AlgCase, style: Style) -> Option<String> {
    let state = case_origins(set, case)?;
    let size = style.pixels;
    let ratio = if style.far_side {
        diagram_ratio(set)
    } else {
        1
    };
    let (width, view_width) = (size * ratio, VIEW * ratio);
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}px" height="{size}px" viewBox="0 0 {view_width} {VIEW}">"#
    );
    match set.model() {
        Model::Stickers(model) => {
            let fills = sticker_fills(model, set, &state);
            if set.solid_view {
                draw_solid(&mut svg, model, &fills, style);
            } else if set.event == WcaEvent::Megaminx {
                let layer = set.model().region("layer:U")?.places;
                draw_folded_top(&mut svg, model, &fills, &layer, style);
            } else {
                draw_top(&mut svg, model, &fills, style);
            }
        }
        Model::Square1 => draw_square1(&mut svg, &state, style),
        Model::Clock => draw_clock(&mut svg, &state, style),
    }
    svg.push_str("</svg>");
    Some(svg)
}

#[cfg(test)]
mod tests {
    use super::super::{OLL, PLL};
    use super::*;
    use crate::library::library_sets;

    fn fills(svg: &str, fill: &str) -> usize {
        svg.matches(&format!(r#"fill="{fill}""#)).count()
    }

    #[test]
    fn every_diagram_has_nine_stickers_on_top_and_twelve_around() {
        for set in [&PLL, &OLL] {
            for case in set.cases {
                let svg = case_diagram(set, case).unwrap();
                assert_eq!(svg.matches("<rect").count(), 21, "{}", case.name);
            }
        }
    }

    #[test]
    fn pll_shows_a_full_top_and_colored_sides() {
        let svg = case_diagram(&PLL, PLL.case("T").unwrap()).unwrap();
        assert_eq!(fills(&svg, TOP), 9);
        assert_eq!(fills(&svg, BLANK), 0);
        for side in [FRONT, RIGHT, BACK, LEFT] {
            assert_eq!(fills(&svg, side), 3);
        }
    }

    #[test]
    fn oll_only_tells_what_faces_up() {
        // Stickers of the last layer are on top or on a side, nine in all.
        let svg = case_diagram(&OLL, OLL.case("OLL 27").unwrap()).unwrap();
        assert_eq!(fills(&svg, TOP), 9);
        assert_eq!(fills(&svg, FRONT), 0);
    }

    #[test]
    fn every_case_of_every_puzzle_can_be_drawn() {
        for set in library_sets() {
            for case in set.cases {
                let svg = case_diagram(set, case);
                let svg = svg.unwrap_or_else(|| panic!("{} {}", set.id, case.name));
                assert!(svg.len() > 200, "{} {}", set.id, case.name);
                assert!(!svg.contains("NaN"), "{} {}", set.id, case.name);
            }
        }
    }

    #[test]
    fn every_case_of_every_puzzle_has_a_thumbnail() {
        for set in library_sets() {
            for case in set.cases {
                let svg = case_thumbnail(set, case);
                let svg = svg.unwrap_or_else(|| panic!("{} {}", set.id, case.name));
                assert!(!svg.contains("NaN"), "{} {}", set.id, case.name);
            }
        }
    }

    #[test]
    fn a_thumbnail_leaves_out_what_is_too_small_to_see() {
        let clock = crate::library::find_set("clock-flips").unwrap();
        let svg = case_thumbnail(clock, &clock.cases[0]).unwrap();
        assert_eq!(svg.matches("<polygon").count(), 9);
        let solid = crate::library::find_set("f2l").unwrap();
        let full = case_diagram(solid, &solid.cases[0]).unwrap();
        let small = case_thumbnail(solid, &solid.cases[0]).unwrap();
        assert!(small.matches("<polygon").count() < full.matches("<polygon").count());
    }

    #[test]
    fn a_thumbnail_keeps_the_gaps_of_a_big_cube_small() {
        let set = crate::library::find_set("7x7-parity").unwrap();
        let svg = case_thumbnail(set, &set.cases[0]).unwrap();
        assert_eq!(svg.matches("<rect").count(), 49 + 28);
    }

    #[test]
    fn a_bigger_cube_draws_more_stickers() {
        let set = crate::library::find_set("4x4-oll-parity").unwrap();
        let svg = case_diagram(set, &set.cases[0]).unwrap();
        assert_eq!(svg.matches("<rect").count(), 16 + 16);
    }

    #[test]
    fn a_megaminx_shows_its_top_layer_from_above() {
        let set = crate::library::find_set("mega-pll-a").unwrap();
        let svg = case_diagram(set, &set.cases[0]).unwrap();
        // Eleven stickers on top, and three on each of the five sides.
        assert_eq!(svg.matches("<polygon").count(), 11 + 5 * 3);
        assert_eq!(fills(&svg, TOP), 11);
        assert_eq!(fills(&svg, BLANK), 0);
        for side in ["F", "R", "BR", "BL", "L"] {
            assert_eq!(fills(&svg, face_color(side)), 3, "{side}");
        }
    }

    #[test]
    fn a_megaminx_side_folds_out_beyond_the_top() {
        let Model::Stickers(model) = Model::of(WcaEvent::Megaminx) else {
            unreachable!()
        };
        let front = model
            .face_names
            .iter()
            .position(|&name| name == "F")
            .unwrap();
        let reach = |point: [f64; 2]| point[0].hypot(point[1]);
        let top_edge = model.stickers[0]
            .polygon
            .iter()
            .map(|&point| folded_out(point, [0., 1., 0.]))
            .map(reach)
            .fold(0., f64::max);
        for sticker in &model.stickers {
            if usize::from(sticker.face) == front && sticker.center[1] > 0.62 {
                let [x, z] = folded_out(sticker.center, sticker.normal);
                assert!(reach([x, z]) > top_edge * 0.8 && z > 0., "{x} {z}");
            }
        }
    }

    #[test]
    fn a_square1_corner_side_goes_out_and_comes_back() {
        let (top, side) = piece_outlines([0., 0.], 10., 0., 2.);
        assert_eq!(top.len(), 4);
        assert_eq!(side.len(), 6);
        // The inner line runs backwards, so its tip comes after its far end.
        assert_eq!(side[3], top[3]);
        assert_eq!(side[4], top[2]);
        assert_eq!(side[5], top[1]);
    }

    #[test]
    fn a_square1_layer_in_cube_shape_has_straight_sides() {
        let (_, edge) = piece_outlines([0., 0.], 10., 1., 2.);
        let (_, corner) = piece_outlines([0., 0.], 10., 2., 4.);
        let (a, b, c) = (edge[0], edge[1], corner[1]);
        let turn = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        assert!(turn.abs() < 0.05, "{turn}");
    }

    #[test]
    fn a_square1_draws_every_piece_of_both_layers() {
        let set = crate::library::find_set("sq1-cubeshape").unwrap();
        let svg = case_diagram(set, &set.cases[0]).unwrap();
        // Sixteen pieces of two polygons each.
        assert_eq!(svg.matches("<polygon").count(), 32);
    }

    #[test]
    fn a_clock_draws_both_faces() {
        let set = crate::library::find_set("clock-flips").unwrap();
        let svg = case_diagram(set, &set.cases[0]).unwrap();
        assert_eq!(svg.matches("<polygon").count(), 18);
        assert!(svg.contains(CLOCK_FRONT.noon) && svg.contains(CLOCK_BACK.noon));
        assert!(svg.contains(r#"viewBox="0 0 200 100""#));
        assert_eq!(diagram_ratio(set), 2);
    }

    #[test]
    fn a_clock_hand_points_at_its_hour() {
        let set = crate::library::find_set("clock-flips").unwrap();
        let svg = case_diagram(set, set.case("UR").unwrap()).unwrap();
        // The case is what `UR1+` solves, so the dials it turns show 11.
        assert_eq!(svg.matches("rotate(330)").count(), 4);
        assert_eq!(svg.matches("rotate(30)").count(), 1);
    }
}
