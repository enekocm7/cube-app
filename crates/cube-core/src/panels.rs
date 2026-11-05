//! Widths of the two side panels beside the timer, and how they fit a window.
//!
//! The UI owns the drag; this module owns the limits, so a width dragged by the
//! user and a width read back from the settings file are held to the same rules.

/// The session and solve list's width before the user changes it.
pub const DEFAULT_SIDEBAR_WIDTH: u32 = 300;
/// The statistics panel's width before the user changes it.
pub const DEFAULT_STATS_WIDTH: u32 = 250;
/// The narrowest either panel may be made, in pixels.
pub const MIN_PANEL_WIDTH: u32 = 180;
/// The widest either panel may be made, in pixels, however wide the window is.
pub const MAX_PANEL_WIDTH: u32 = 350;
/// Room the timer keeps free, so the panels cannot take the whole window.
pub const MIN_TIMER_WIDTH: u32 = 320;
/// The width a panel needs to show its columns at the normal text size. Larger
/// text raises a panel's minimum from this, in proportion.
const READABLE_PANEL_WIDTH: f32 = 200.;
/// The width a row of the statistics panel needs at the normal text size to
/// show its label and two values side by side.
const STATS_ROW_WIDTH: f32 = 196.;

/// The narrowest a panel may be in a window `window_width` pixels wide, with
/// text `text_scale` times the normal size.
///
/// Larger text needs a wider panel, but both panels must still fit beside the
/// timer, so the window has the last word. Never below [`MIN_PANEL_WIDTH`].
pub fn min_panel_width(window_width: u32, text_scale: f32) -> u32 {
    if text_scale <= 1. {
        return MIN_PANEL_WIDTH;
    }
    let readable = (READABLE_PANEL_WIDTH * text_scale).ceil() as u32;
    let share = window_width.saturating_sub(MIN_TIMER_WIDTH) / 2;
    readable.min(share).max(MIN_PANEL_WIDTH)
}

/// The widest a panel can be in a window `window_width` pixels wide, while the
/// other panel is `other_width` pixels wide and text is `text_scale` times the
/// normal size.
///
/// The room left for it is the window less the other panel and the timer's
/// minimum, and never more than [`MAX_PANEL_WIDTH`], which grows with larger
/// text. Never narrower than [`min_panel_width`], even when the window cannot
/// fit that room: the panel keeps its minimum and the window clips instead of
/// the panel collapsing.
pub fn max_panel_width(window_width: u32, other_width: u32, text_scale: f32) -> u32 {
    let min = min_panel_width(window_width, text_scale);
    let max = (MAX_PANEL_WIDTH as f32 * text_scale.max(1.)).ceil() as u32;
    window_width
        .saturating_sub(other_width)
        .saturating_sub(MIN_TIMER_WIDTH)
        .clamp(min, max.max(min))
}

/// Clamps `width` to the range a panel may take beside the other panel.
pub fn clamp_panel_width(width: u32, window_width: u32, other_width: u32, text_scale: f32) -> u32 {
    width.clamp(
        min_panel_width(window_width, text_scale),
        max_panel_width(window_width, other_width, text_scale),
    )
}

/// Returns the sidebar and statistics widths to draw in a window `window_width`
/// pixels wide, with text `text_scale` times the normal size.
///
/// The statistics panel is first held to its own range with the sidebar at its
/// minimum, then the sidebar is fitted beside it, then the statistics panel is
/// fitted beside the sidebar. Settling the statistics panel first matters: a
/// stored width below the minimum must not let the sidebar take room that the
/// statistics panel then cannot give back. The two never take more room between
/// them than the window leaves after the timer, unless the window cannot hold
/// both minimums.
pub fn fit_panels(window_width: u32, sidebar: u32, stats: u32, text_scale: f32) -> (u32, u32) {
    let min = min_panel_width(window_width, text_scale);
    let stats = clamp_panel_width(stats, window_width, min, text_scale);
    let sidebar = clamp_panel_width(sidebar, window_width, stats, text_scale);
    let stats = clamp_panel_width(stats, window_width, sidebar, text_scale);
    (sidebar, stats)
}

/// Whether the statistics panel, `panel_width` pixels wide with text
/// `text_scale` times the normal size, must put each label above its values
/// because the three no longer fit side by side.
pub fn stats_rows_stack(panel_width: u32, text_scale: f32) -> bool {
    (panel_width as f32) < STATS_ROW_WIDTH * text_scale
}

/// The font size that keeps text within `room` pixels and `max_height` pixels
/// tall, for text `text_width` pixels wide at `font_px`.
///
/// Text that already fits keeps its size, so this only ever shrinks.
pub fn fit_font_px(font_px: f32, text_width: f32, room: f32, max_height: f32) -> f32 {
    let fitted = if text_width > room && text_width > 0. {
        font_px * room.max(0.) / text_width
    } else {
        font_px
    };
    fitted.min(max_height.max(0.))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_widths_fit_a_full_hd_window() {
        assert_eq!(
            fit_panels(1920, DEFAULT_SIDEBAR_WIDTH, DEFAULT_STATS_WIDTH, 1.),
            (DEFAULT_SIDEBAR_WIDTH, DEFAULT_STATS_WIDTH)
        );
    }

    #[test]
    fn default_widths_fit_the_minimum_window() {
        // The window's minimum size in `run`.
        assert_eq!(
            fit_panels(910, DEFAULT_SIDEBAR_WIDTH, DEFAULT_STATS_WIDTH, 1.),
            (DEFAULT_SIDEBAR_WIDTH, DEFAULT_STATS_WIDTH)
        );
    }

    #[test]
    fn panels_never_go_below_the_minimum() {
        assert_eq!(clamp_panel_width(0, 1920, 250, 1.), MIN_PANEL_WIDTH);
        assert_eq!(clamp_panel_width(10, 1920, 250, 1.), MIN_PANEL_WIDTH);
    }

    #[test]
    fn the_window_limits_the_widest_panel() {
        // 900 px, less the other panel and the timer's room.
        assert_eq!(max_panel_width(900, 250, 1.), 330);
        assert_eq!(clamp_panel_width(2000, 900, 250, 1.), 330);
    }

    #[test]
    fn a_panel_never_exceeds_the_maximum() {
        assert_eq!(max_panel_width(5000, 250, 1.), MAX_PANEL_WIDTH);
        assert_eq!(clamp_panel_width(4000, 5000, 250, 1.), MAX_PANEL_WIDTH);
        // A saved width above the maximum is clamped, not rejected.
        assert_eq!(
            fit_panels(5000, 3000, 2000, 1.),
            (MAX_PANEL_WIDTH, MAX_PANEL_WIDTH)
        );
    }

    #[test]
    fn a_narrow_window_keeps_the_minimum() {
        assert_eq!(max_panel_width(300, 250, 1.), MIN_PANEL_WIDTH);
        assert_eq!(
            fit_panels(300, 300, 300, 1.),
            (MIN_PANEL_WIDTH, MIN_PANEL_WIDTH)
        );
    }

    #[test]
    fn a_wide_sidebar_gives_way_to_the_stats_panel() {
        // The sidebar is capped by the stats panel; the stats panel then fits
        // beside the capped sidebar.
        assert_eq!(fit_panels(800, 600, 250, 1.), (230, 250));
    }

    #[test]
    fn fitted_panels_leave_the_timer_its_room() {
        for window_width in (0..=2000).step_by(37) {
            for sidebar in (0..=800).step_by(53) {
                for stats in (0..=800).step_by(59) {
                    let (sidebar, stats) = fit_panels(window_width, sidebar, stats, 1.);
                    assert!(sidebar >= MIN_PANEL_WIDTH);
                    assert!(stats >= MIN_PANEL_WIDTH);
                    assert!(sidebar <= MAX_PANEL_WIDTH && stats <= MAX_PANEL_WIDTH);
                    assert!(
                        window_width < 2 * MIN_PANEL_WIDTH + MIN_TIMER_WIDTH
                            || sidebar + stats + MIN_TIMER_WIDTH <= window_width,
                        "{window_width}: {sidebar} + {stats} leaves no timer room"
                    );
                }
            }
        }
    }

    #[test]
    fn normal_and_smaller_text_keep_the_plain_minimum() {
        assert_eq!(min_panel_width(1920, 1.), MIN_PANEL_WIDTH);
        assert_eq!(min_panel_width(1920, 0.83), MIN_PANEL_WIDTH);
    }

    #[test]
    fn larger_text_widens_the_panels() {
        // 24 pt text is twice the normal size.
        assert_eq!(min_panel_width(1920, 2.), 400);
        assert_eq!(fit_panels(1920, 300, 250, 2.), (400, 400));
        assert_eq!(max_panel_width(5000, 250, 2.), 700);
    }

    #[test]
    fn the_largest_preset_keeps_the_default_widths() {
        assert_eq!(
            fit_panels(910, DEFAULT_SIDEBAR_WIDTH, DEFAULT_STATS_WIDTH, 1.25),
            (DEFAULT_SIDEBAR_WIDTH, DEFAULT_STATS_WIDTH)
        );
    }

    #[test]
    fn the_window_limits_how_far_text_widens_the_panels() {
        // Half of what the timer leaves in the smallest window.
        assert_eq!(min_panel_width(910, 2.), 295);
        let (sidebar, stats) = fit_panels(910, 300, 250, 2.);
        assert_eq!((sidebar, stats), (295, 295));
        assert!(sidebar + stats + MIN_TIMER_WIDTH <= 910);
    }

    #[test]
    fn stats_rows_stay_on_one_line_at_the_default_width() {
        assert!(!stats_rows_stack(DEFAULT_STATS_WIDTH, 0.83));
        assert!(!stats_rows_stack(DEFAULT_STATS_WIDTH, 1.));
        assert!(!stats_rows_stack(DEFAULT_STATS_WIDTH, 1.25));
    }

    #[test]
    fn stats_rows_stay_on_one_line_when_the_panels_can_widen() {
        let (_, stats) = fit_panels(1180, 300, 250, 2.);
        assert!(!stats_rows_stack(stats, 2.));
    }

    #[test]
    fn stats_rows_stack_when_the_window_keeps_the_panel_narrow() {
        let (_, stats) = fit_panels(910, 300, 250, 2.);
        assert!(stats_rows_stack(stats, 2.));
    }

    #[test]
    fn text_that_fits_keeps_its_size() {
        assert!((fit_font_px(100., 300., 400., 500.) - 100.).abs() < f32::EPSILON);
        assert!((fit_font_px(100., 0., 0., 500.) - 100.).abs() < f32::EPSILON);
    }

    #[test]
    fn text_too_wide_shrinks_to_the_room() {
        assert!((fit_font_px(100., 400., 200., 500.) - 50.).abs() < f32::EPSILON);
    }

    #[test]
    fn text_too_tall_shrinks_to_the_height() {
        assert!((fit_font_px(320., 300., 400., 200.) - 200.).abs() < f32::EPSILON);
    }

    #[test]
    fn no_room_never_gives_a_negative_size() {
        assert!(fit_font_px(100., 400., -20., 500.).abs() < f32::EPSILON);
        assert!(fit_font_px(100., 400., 200., -1.).abs() < f32::EPSILON);
    }
}
