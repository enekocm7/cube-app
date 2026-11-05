//! Sizes of the GUI's dialogs, relative to the window showing them.
//!
//! The UI draws the surfaces; this module owns how much of the window each one
//! may take, so every dialog grows and shrinks by the same rules and the
//! arithmetic can be tested without GPUI.

/// Space kept between a dialog's scrolling body and the divider above its
/// footer, so the last row never sits flush against the line.
///
/// Padding inside a scroll region scrolls away, so this gap is reserved
/// outside the viewport and taken out of the body height.
pub const BODY_INSET: f32 = 12.;

/// The size rules of one dialog: a share of the window, kept between a floor
/// and a ceiling.
#[derive(Clone, Copy, Debug)]
pub struct DialogSize {
    /// Share of the window's width the dialog asks for.
    pub width_share: f32,
    /// Narrowest the dialog may get when the window is wide enough.
    pub min_width: f32,
    /// Widest the dialog may get, so it does not stretch across a large screen.
    pub max_width: f32,
    /// Share of the window's height the scrolling body asks for.
    pub body_share: f32,
    /// Shortest the body may get when the window is tall enough.
    pub min_body_height: f32,
    /// Tallest the body may get.
    pub max_body_height: f32,
    /// Room the dialog needs for its title, footer and window margins.
    pub chrome_height: f32,
}

impl DialogSize {
    /// Short confirmations and prompts.
    pub const COMPACT: Self = Self {
        width_share: 0.4,
        min_width: 400.,
        max_width: 560.,
        body_share: 0.5,
        min_body_height: 180.,
        max_body_height: 360.,
        chrome_height: 160.,
    };

    /// The release notes and updater dialogs.
    pub const STANDARD: Self = Self {
        width_share: 0.5,
        min_width: 480.,
        max_width: 900.,
        body_share: 0.65,
        min_body_height: 240.,
        max_body_height: 760.,
        chrome_height: 170.,
    };

    /// The solve editor, whose fields sit side by side.
    pub const FORM: Self = Self {
        width_share: 0.54,
        min_width: 520.,
        max_width: 840.,
        body_share: 0.7,
        min_body_height: 260.,
        max_body_height: 780.,
        chrome_height: 220.,
    };

    /// The theme editor, with its color controls and preview.
    pub const WIDE: Self = Self {
        width_share: 0.62,
        min_width: 760.,
        max_width: 1080.,
        body_share: 0.7,
        min_body_height: 260.,
        max_body_height: 820.,
        chrome_height: 190.,
    };

    /// The settings dialog, with its category list beside the settings. It has
    /// no title or footer of its own, so nearly all of it is body.
    pub const SETTINGS: Self = Self {
        width_share: 0.72,
        min_width: 720.,
        max_width: 1120.,
        body_share: 0.76,
        min_body_height: 320.,
        max_body_height: 860.,
        chrome_height: 64.,
    };

    /// The dialog's width in a window `viewport_width` pixels wide.
    ///
    /// A window narrower than the minimum gets its whole width, leaving the
    /// dialog's own margins to keep it on screen, instead of a width that
    /// would overflow.
    pub fn width(self, viewport_width: f32) -> f32 {
        (viewport_width * self.width_share)
            .clamp(self.min_width, self.max_width)
            .min(viewport_width)
    }

    /// The tallest the scrolling body may get in a window `viewport_height`
    /// pixels tall.
    ///
    /// It is a share of the window, so a large monitor shows more rows, while a
    /// small one scrolls the body instead of pushing the footer out of the
    /// dialog.
    pub fn body_height(self, viewport_height: f32) -> f32 {
        let room = (viewport_height - self.chrome_height - BODY_INSET).max(0.0);
        (viewport_height * self.body_share)
            .clamp(self.min_body_height, self.max_body_height)
            .min(room)
    }
}

/// The width of the settings dialog's category list in a dialog `dialog_width`
/// pixels wide.
pub fn settings_nav_width(dialog_width: f32) -> f32 {
    (dialog_width * 0.24).clamp(180., 260.).min(dialog_width)
}

/// How far a scroll region must move so a row it holds comes into view, keeping
/// `margin` clear of the region's edges. Positive moves the content down.
///
/// A row taller than the region lines up with its top, so its label shows.
pub fn reveal_shift(row: (f32, f32), region: (f32, f32), margin: f32) -> f32 {
    let (row_top, row_bottom) = row;
    let (top, bottom) = (region.0 + margin, region.1 - margin);
    if row_top < top || row_bottom - row_top > bottom - top {
        top - row_top
    } else if row_bottom > bottom {
        bottom - row_bottom
    } else {
        0.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZES: [DialogSize; 5] = [
        DialogSize::COMPACT,
        DialogSize::STANDARD,
        DialogSize::FORM,
        DialogSize::WIDE,
        DialogSize::SETTINGS,
    ];

    fn close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.01,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn a_dialog_grows_with_the_window_then_stops_at_its_maximum() {
        close(DialogSize::STANDARD.width(1180.0), 590.0);
        close(DialogSize::STANDARD.width(1920.0), 900.0);
        assert!(DialogSize::WIDE.width(1920.0) > DialogSize::WIDE.width(1180.0));
    }

    #[test]
    fn a_narrow_window_holds_a_dialog_to_its_minimum_and_never_overflows() {
        close(DialogSize::STANDARD.width(700.0), 480.0);
        for size in SIZES {
            for viewport in [200.0, 400.0, 910.0] {
                assert!(
                    size.width(viewport) <= viewport,
                    "{viewport}: {} overflows the window",
                    size.width(viewport)
                );
            }
        }
    }

    #[test]
    fn the_body_is_proportional_and_grows_with_the_window() {
        close(DialogSize::STANDARD.body_height(760.0), 494.0);
        close(DialogSize::STANDARD.body_height(2160.0), 760.0);
        assert!(DialogSize::WIDE.body_height(2160.0) > DialogSize::WIDE.body_height(760.0));
    }

    #[test]
    fn the_body_leaves_room_for_the_dialog_chrome() {
        for size in SIZES {
            for viewport in [0.0, 120.0, 560.0, 760.0, 2160.0] {
                let body = size.body_height(viewport);
                let room = (viewport - size.chrome_height - BODY_INSET).max(0.0);
                assert!(
                    body <= room,
                    "{viewport}: {body} does not fit in the {room} left for a body"
                );
            }
        }
    }

    #[test]
    fn a_tiny_window_leaves_no_body_room_instead_of_a_negative_height() {
        close(DialogSize::STANDARD.body_height(100.0), 0.0);
    }

    #[test]
    fn the_settings_dialog_fits_the_smallest_window() {
        let (width, height) = (910.0, 560.0);
        assert!(DialogSize::SETTINGS.width(width) <= width);
        assert!(DialogSize::SETTINGS.body_height(height) >= 320.0);
    }

    #[test]
    fn the_category_list_stays_a_readable_share_of_the_dialog() {
        close(settings_nav_width(720.0), 180.0);
        close(settings_nav_width(900.0), 216.0);
        close(settings_nav_width(1120.0), 260.0);
        close(settings_nav_width(100.0), 100.0);
    }

    #[test]
    fn a_row_in_view_does_not_move_the_region() {
        close(reveal_shift((120.0, 160.0), (100.0, 300.0), 10.0), 0.0);
    }

    #[test]
    fn a_row_past_an_edge_moves_just_far_enough_to_clear_the_margin() {
        close(reveal_shift((80.0, 120.0), (100.0, 300.0), 10.0), 30.0);
        close(reveal_shift((280.0, 320.0), (100.0, 300.0), 10.0), -30.0);
    }

    #[test]
    fn a_row_taller_than_the_region_shows_its_top() {
        close(reveal_shift((150.0, 500.0), (100.0, 300.0), 10.0), -40.0);
    }
}
