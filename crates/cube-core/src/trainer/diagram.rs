//! A top-down drawing of a case's last layer, small enough for a grid of them.

use std::fmt::Write as _;

use super::cube::{facelets, setup};
use super::{AlgCase, AlgSet, Step};

/// How wide and tall the drawing is, in logical pixels.
const VIEW: u32 = 100;

/// How many times larger than its natural size the SVG is rasterized, so that
/// it stays sharp on high-density screens.
const RASTER_SCALE: u32 = 3;

/// Space around a sticker, in the units of the view.
const GAP: f32 = 1.5;

/// How deep the strips showing the sides are.
const STRIP: f32 = 11.;

/// The last layer's color. Cases are drawn the way they are solved: with that
/// layer on top, and green at the front.
const TOP: &str = "#f5d90a";
const FRONT: &str = "#30a46c";
const RIGHT: &str = "#f76b15";
const BACK: &str = "#0091ff";
const LEFT: &str = "#e5484d";

/// A sticker that says nothing about the case.
const BLANK: &str = "#7e868c";

/// The side stickers along each edge of the top face, as positions in the
/// facelets, from left to right or from back to front as drawn.
const BACK_ROW: [usize; 3] = [47, 46, 45];
const FRONT_ROW: [usize; 3] = [18, 19, 20];
const LEFT_COLUMN: [usize; 3] = [36, 37, 38];
const RIGHT_COLUMN: [usize; 3] = [11, 10, 9];

/// The color of a sticker that began on the given face.
fn color(step: Step, sticker: u8) -> &'static str {
    match (step, sticker) {
        (_, b'U') => TOP,
        (Step::PermuteLastLayer, b'F') => FRONT,
        (Step::PermuteLastLayer, b'R') => RIGHT,
        (Step::PermuteLastLayer, b'B') => BACK,
        (Step::PermuteLastLayer, b'L') => LEFT,
        _ => BLANK,
    }
}

fn rectangle(svg: &mut String, [x, y, width, height]: [f32; 4], fill: &str) {
    let _ = write!(
        svg,
        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="1.5" fill="{fill}"/>"#,
        x + GAP / 2.,
        y + GAP / 2.,
        width - GAP,
        height - GAP
    );
}

/// Draws `case` as an SVG document, or `None` when its algorithm cannot be
/// read, which the tests rule out for the built-in sets.
pub fn case_diagram(set: &AlgSet, case: &AlgCase) -> Option<String> {
    let cube = facelets(&setup(case).ok()?);
    let cube = cube.as_bytes();
    let step = set.step;
    let size = RASTER_SCALE * VIEW;
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{size}px" height="{size}px" viewBox="0 0 {VIEW} {VIEW}">"#
    );

    let view = VIEW as f32;
    let cell = (view - 2. * STRIP) / 3.;
    let place = |index: usize| index as f32 * cell + STRIP;
    for row in 0..3 {
        for column in 0..3 {
            let sticker = cube[row * 3 + column];
            let area = [place(column), place(row), cell, cell];
            rectangle(&mut svg, area, color(step, sticker));
        }
    }
    for index in 0..3 {
        let along = place(index);
        let sides = [
            (BACK_ROW, [along, 0., cell, STRIP]),
            (FRONT_ROW, [along, view - STRIP, cell, STRIP]),
            (LEFT_COLUMN, [0., along, STRIP, cell]),
            (RIGHT_COLUMN, [view - STRIP, along, STRIP, cell]),
        ];
        for (stickers, area) in sides {
            rectangle(&mut svg, area, color(step, cube[stickers[index]]));
        }
    }
    svg.push_str("</svg>");
    Some(svg)
}

#[cfg(test)]
mod tests {
    use super::super::{OLL, PLL};
    use super::*;

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
    fn the_side_stickers_are_drawn_where_they_are() {
        // After U the front row shows what began on the right.
        let cube = facelets(&super::super::notation::parse("U").unwrap());
        let row = FRONT_ROW.map(|sticker| cube.as_bytes()[sticker]);
        assert_eq!(row, [b'R'; 3]);
        let column = RIGHT_COLUMN.map(|sticker| cube.as_bytes()[sticker]);
        assert_eq!(column, [b'B'; 3]);
    }

    #[test]
    fn the_columns_run_from_back_to_front() {
        // R turns the front right corner's side sticker onto the back.
        let cube = facelets(&super::super::notation::parse("F").unwrap());
        // After F the left column's front sticker moved, the back one did not.
        assert_eq!(cube.as_bytes()[LEFT_COLUMN[0]], b'L');
        assert_ne!(cube.as_bytes()[LEFT_COLUMN[2]], b'L');
        assert_eq!(cube.as_bytes()[RIGHT_COLUMN[0]], b'R');
        assert_ne!(cube.as_bytes()[RIGHT_COLUMN[2]], b'R');
    }
}
