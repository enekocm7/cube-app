//! Widths of the two side panels beside the timer, and how they fit a window.
//!
//! The UI owns the drag and the measuring; this module owns the limits, so a
//! width dragged by the user and a width read back from the settings file are
//! held to the same rules.

/// The session and solve list's width before the user changes it.
pub const DEFAULT_SIDEBAR_WIDTH: u32 = 300;
/// The statistics panel's width before the user changes it.
pub const DEFAULT_STATS_WIDTH: u32 = 250;
/// The narrowest either panel may be made, in pixels, whatever its content.
pub const MIN_PANEL_WIDTH: u32 = 180;
/// The widest either panel may be made, in pixels, however wide the window is.
pub const MAX_PANEL_WIDTH: u32 = 350;
/// Room the timer keeps free, so the panels cannot take the whole window.
pub const MIN_TIMER_WIDTH: u32 = 320;

/// How wide each panel's content is, in pixels, as measured by the UI with the
/// real font, text size, language and the longest values on show.
///
/// A width of `0` means nothing was measured, and the panel only keeps
/// [`MIN_PANEL_WIDTH`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PanelContent {
    /// The session and solve list: number, widest time and ao5 columns.
    pub sidebar: u32,
    /// One row of the statistics panel: label and both value columns.
    pub stats: u32,
}

/// The narrowest each panel may be, as `(sidebar, stats)`, in a window
/// `window_width` pixels wide.
///
/// Each panel needs the width of its content, never less than
/// [`MIN_PANEL_WIDTH`]. Both must still fit beside the timer, so when the
/// window cannot hold both, the narrower need is kept and the other panel gets
/// the rest, or both share the room equally.
pub fn panel_minimums(window_width: u32, content: PanelContent) -> (u32, u32) {
    let room = window_width.saturating_sub(MIN_TIMER_WIDTH);
    let half = room / 2;
    let (mut sidebar, mut stats) = (content.sidebar, content.stats);
    if sidebar + stats > room {
        if sidebar <= half {
            stats = room - sidebar;
        } else if stats <= half {
            sidebar = room - stats;
        } else {
            (sidebar, stats) = (half, half);
        }
    }
    (sidebar.max(MIN_PANEL_WIDTH), stats.max(MIN_PANEL_WIDTH))
}

/// The widest a panel can be in a window `window_width` pixels wide, while the
/// other panel is `other_width` pixels wide, text is `text_scale` times the
/// normal size and the panel's own minimum is `min`.
///
/// The room left for it is the window less the other panel and the timer's
/// minimum, and never more than [`MAX_PANEL_WIDTH`], which grows with larger
/// text. Never narrower than `min`, even when the window cannot fit that room:
/// the panel keeps its minimum and the window clips instead of the panel
/// collapsing.
pub fn max_panel_width(window_width: u32, other_width: u32, text_scale: f32, min: u32) -> u32 {
    let max = (MAX_PANEL_WIDTH as f32 * text_scale.max(1.)).ceil() as u32;
    window_width
        .saturating_sub(other_width)
        .saturating_sub(MIN_TIMER_WIDTH)
        .clamp(min, max.max(min))
}

/// Clamps `width` to the range a panel may take beside the other panel, given
/// its own minimum `min` from [`panel_minimums`].
pub fn clamp_panel_width(
    width: u32,
    window_width: u32,
    other_width: u32,
    text_scale: f32,
    min: u32,
) -> u32 {
    width.clamp(
        min,
        max_panel_width(window_width, other_width, text_scale, min),
    )
}

/// Returns the sidebar and statistics widths to draw in a window `window_width`
/// pixels wide, with text `text_scale` times the normal size and panels whose
/// content is `content` wide.
///
/// The statistics panel is first held to its own range with the sidebar at its
/// minimum, then the sidebar is fitted beside it, then the statistics panel is
/// fitted beside the sidebar. Settling the statistics panel first matters: a
/// stored width below the minimum must not let the sidebar take room that the
/// statistics panel then cannot give back. The two never take more room between
/// them than the window leaves after the timer, unless the window cannot hold
/// both minimums.
pub fn fit_panels(
    window_width: u32,
    sidebar: u32,
    stats: u32,
    text_scale: f32,
    content: PanelContent,
) -> (u32, u32) {
    let (sidebar_min, stats_min) = panel_minimums(window_width, content);
    let stats = clamp_panel_width(stats, window_width, sidebar_min, text_scale, stats_min);
    let sidebar = clamp_panel_width(sidebar, window_width, stats, text_scale, sidebar_min);
    let stats = clamp_panel_width(stats, window_width, sidebar, text_scale, stats_min);
    (sidebar, stats)
}

/// Whether the statistics panel, `panel_width` pixels wide, must put each label
/// above its values because a row, `row_width` pixels wide, no longer fits
/// side by side.
pub fn stats_rows_stack(panel_width: u32, row_width: u32) -> bool {
    panel_width < row_width
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

    const NONE: PanelContent = PanelContent {
        sidebar: 0,
        stats: 0,
    };
    const NORMAL: PanelContent = PanelContent {
        sidebar: 200,
        stats: 196,
    };

    #[test]
    fn default_widths_fit_a_full_hd_window() {
        assert_eq!(
            fit_panels(1920, DEFAULT_SIDEBAR_WIDTH, DEFAULT_STATS_WIDTH, 1., NORMAL),
            (DEFAULT_SIDEBAR_WIDTH, DEFAULT_STATS_WIDTH)
        );
    }

    #[test]
    fn default_widths_fit_the_minimum_window() {
        // The window's minimum size in `run`.
        assert_eq!(
            fit_panels(910, DEFAULT_SIDEBAR_WIDTH, DEFAULT_STATS_WIDTH, 1., NORMAL),
            (DEFAULT_SIDEBAR_WIDTH, DEFAULT_STATS_WIDTH)
        );
    }

    #[test]
    fn unmeasured_panels_keep_the_floor() {
        assert_eq!(
            panel_minimums(1920, NONE),
            (MIN_PANEL_WIDTH, MIN_PANEL_WIDTH)
        );
        assert_eq!(
            clamp_panel_width(0, 1920, 250, 1., MIN_PANEL_WIDTH),
            MIN_PANEL_WIDTH
        );
    }

    #[test]
    fn panels_never_go_below_their_content() {
        let content = PanelContent {
            sidebar: 230,
            stats: 260,
        };
        assert_eq!(panel_minimums(1920, content), (230, 260));
        assert_eq!(fit_panels(1920, 0, 0, 1., content), (230, 260));
        // Stored widths below the minimum are raised.
        assert_eq!(fit_panels(1920, 181, 182, 1., content), (230, 260));
    }

    #[test]
    fn the_stats_panel_cannot_be_dragged_into_stacking() {
        let (_, stats) = fit_panels(1920, 300, 0, 1., NORMAL);
        assert!(!stats_rows_stack(stats, NORMAL.stats));
    }

    #[test]
    fn the_minimum_grows_with_the_content() {
        let wide = PanelContent {
            sidebar: 320,
            stats: 196,
        };
        assert_eq!(panel_minimums(1920, wide).0, 320);
        assert_eq!(fit_panels(1920, 100, 250, 1., wide).0, 320);
    }

    #[test]
    fn a_window_too_small_for_both_minimums_shares_the_room() {
        // 910 px leaves 590 for the panels.
        let big = PanelContent {
            sidebar: 500,
            stats: 500,
        };
        assert_eq!(panel_minimums(910, big), (295, 295));
        // The narrower need is honoured, the other panel gets the rest.
        let uneven = PanelContent {
            sidebar: 500,
            stats: 250,
        };
        assert_eq!(panel_minimums(910, uneven), (340, 250));
        let (sidebar, stats) = fit_panels(910, 300, 250, 1., big);
        assert!(sidebar + stats + MIN_TIMER_WIDTH <= 910);
    }

    #[test]
    fn the_window_limits_the_widest_panel() {
        // 900 px, less the other panel and the timer's room.
        assert_eq!(max_panel_width(900, 250, 1., MIN_PANEL_WIDTH), 330);
        assert_eq!(clamp_panel_width(2000, 900, 250, 1., MIN_PANEL_WIDTH), 330);
    }

    #[test]
    fn a_panel_never_exceeds_the_maximum() {
        assert_eq!(
            max_panel_width(5000, 250, 1., MIN_PANEL_WIDTH),
            MAX_PANEL_WIDTH
        );
        // A saved width above the maximum is clamped, not rejected.
        assert_eq!(
            fit_panels(5000, 3000, 2000, 1., NORMAL),
            (MAX_PANEL_WIDTH, MAX_PANEL_WIDTH)
        );
        // Larger text raises the maximum.
        assert_eq!(max_panel_width(5000, 250, 2., MIN_PANEL_WIDTH), 700);
    }

    #[test]
    fn content_wider_than_the_maximum_still_fits() {
        let huge = PanelContent {
            sidebar: 800,
            stats: 800,
        };
        assert_eq!(fit_panels(5000, 300, 250, 1., huge), (800, 800));
    }

    #[test]
    fn a_narrow_window_keeps_the_minimum() {
        assert_eq!(
            max_panel_width(300, 250, 1., MIN_PANEL_WIDTH),
            MIN_PANEL_WIDTH
        );
        assert_eq!(
            fit_panels(300, 300, 300, 1., NORMAL),
            (MIN_PANEL_WIDTH, MIN_PANEL_WIDTH)
        );
    }

    #[test]
    fn a_wide_sidebar_gives_way_to_the_stats_panel() {
        assert_eq!(fit_panels(800, 600, 250, 1., NONE), (230, 250));
    }

    #[test]
    fn fitted_panels_leave_the_timer_its_room() {
        for window_width in (0..=2000).step_by(37) {
            for sidebar in (0..=800).step_by(53) {
                for stats in (0..=800).step_by(59) {
                    let (sidebar, stats) = fit_panels(window_width, sidebar, stats, 1., NORMAL);
                    assert!(sidebar >= MIN_PANEL_WIDTH);
                    assert!(stats >= MIN_PANEL_WIDTH);
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
    fn stats_rows_stack_only_below_their_width() {
        assert!(!stats_rows_stack(250, 196));
        assert!(!stats_rows_stack(196, 196));
        assert!(stats_rows_stack(195, 196));
    }

    #[test]
    fn stats_rows_stack_when_the_window_keeps_the_panel_narrow() {
        let big = PanelContent {
            sidebar: 500,
            stats: 500,
        };
        let (_, stats) = fit_panels(910, 300, 250, 1., big);
        assert!(stats_rows_stack(stats, big.stats));
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
