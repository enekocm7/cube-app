//! Sizes of the GUI's dialogs, relative to the window showing them.
//!
//! The UI draws the surfaces; this module owns how much of the window each one
//! may take, so every dialog grows and shrinks by the same rules and the
//! arithmetic can be tested without GPUI.

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

    /// The settings, release notes and updater dialogs.
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
        let room = (viewport_height - self.chrome_height).max(0.0);
        (viewport_height * self.body_share)
            .clamp(self.min_body_height, self.max_body_height)
            .min(room)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZES: [DialogSize; 4] = [
        DialogSize::COMPACT,
        DialogSize::STANDARD,
        DialogSize::FORM,
        DialogSize::WIDE,
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
        assert!(DialogSize::WIDE.body_height(2160.0) > DialogSize::WIDE.body_height(760.0));
    }

    #[test]
    fn the_body_leaves_room_for_the_dialog_chrome() {
        for size in SIZES {
            for viewport in [0.0, 120.0, 560.0, 760.0, 2160.0] {
                let body = size.body_height(viewport);
                let room = (viewport - size.chrome_height).max(0.0);
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
}
