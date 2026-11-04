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

/// The widest a panel can be in a window `window_width` pixels wide, while the
/// other panel is `other_width` pixels wide.
///
/// The room left for it is the window less the other panel and the timer's
/// minimum, and never more than [`MAX_PANEL_WIDTH`]. Never narrower than [`MIN_PANEL_WIDTH`], even when the
/// window cannot fit that room: the panel keeps its minimum and the window
/// clips instead of the panel collapsing.
pub fn max_panel_width(window_width: u32, other_width: u32) -> u32 {
    window_width
        .saturating_sub(other_width)
        .saturating_sub(MIN_TIMER_WIDTH)
        .clamp(MIN_PANEL_WIDTH, MAX_PANEL_WIDTH)
}

/// Clamps `width` to the range a panel may take beside the other panel.
pub fn clamp_panel_width(width: u32, window_width: u32, other_width: u32) -> u32 {
    width.clamp(MIN_PANEL_WIDTH, max_panel_width(window_width, other_width))
}

/// Returns the sidebar and statistics widths to draw in a window `window_width`
/// pixels wide.
///
/// The statistics panel is first held to its own range with the sidebar at its
/// minimum, then the sidebar is fitted beside it, then the statistics panel is
/// fitted beside the sidebar. Settling the statistics panel first matters: a
/// stored width below the minimum must not let the sidebar take room that the
/// statistics panel then cannot give back. The two never take more room between
/// them than the window leaves after the timer, unless the window cannot hold
/// both minimums.
pub fn fit_panels(window_width: u32, sidebar: u32, stats: u32) -> (u32, u32) {
    let stats = clamp_panel_width(stats, window_width, MIN_PANEL_WIDTH);
    let sidebar = clamp_panel_width(sidebar, window_width, stats);
    let stats = clamp_panel_width(stats, window_width, sidebar);
    (sidebar, stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_widths_fit_a_full_hd_window() {
        assert_eq!(
            fit_panels(1920, DEFAULT_SIDEBAR_WIDTH, DEFAULT_STATS_WIDTH),
            (DEFAULT_SIDEBAR_WIDTH, DEFAULT_STATS_WIDTH)
        );
    }

    #[test]
    fn default_widths_fit_the_minimum_window() {
        // The window's minimum size in `run`.
        assert_eq!(
            fit_panels(910, DEFAULT_SIDEBAR_WIDTH, DEFAULT_STATS_WIDTH),
            (DEFAULT_SIDEBAR_WIDTH, DEFAULT_STATS_WIDTH)
        );
    }

    #[test]
    fn panels_never_go_below_the_minimum() {
        assert_eq!(clamp_panel_width(0, 1920, 250), MIN_PANEL_WIDTH);
        assert_eq!(clamp_panel_width(10, 1920, 250), MIN_PANEL_WIDTH);
    }

    #[test]
    fn the_window_limits_the_widest_panel() {
        // 900 px, less the other panel and the timer's room.
        assert_eq!(max_panel_width(900, 250), 330);
        assert_eq!(clamp_panel_width(2000, 900, 250), 330);
    }

    #[test]
    fn a_panel_never_exceeds_the_maximum() {
        assert_eq!(max_panel_width(5000, 250), MAX_PANEL_WIDTH);
        assert_eq!(clamp_panel_width(4000, 5000, 250), MAX_PANEL_WIDTH);
        // A saved width above the maximum is clamped, not rejected.
        assert_eq!(
            fit_panels(5000, 3000, 2000),
            (MAX_PANEL_WIDTH, MAX_PANEL_WIDTH)
        );
    }

    #[test]
    fn a_narrow_window_keeps_the_minimum() {
        assert_eq!(max_panel_width(300, 250), MIN_PANEL_WIDTH);
        assert_eq!(
            fit_panels(300, 300, 300),
            (MIN_PANEL_WIDTH, MIN_PANEL_WIDTH)
        );
    }

    #[test]
    fn a_wide_sidebar_gives_way_to_the_stats_panel() {
        // The sidebar is capped by the stats panel; the stats panel then fits
        // beside the capped sidebar.
        assert_eq!(fit_panels(800, 600, 250), (230, 250));
    }

    #[test]
    fn fitted_panels_leave_the_timer_its_room() {
        for window_width in (0..=2000).step_by(37) {
            for sidebar in (0..=800).step_by(53) {
                for stats in (0..=800).step_by(59) {
                    let (sidebar, stats) = fit_panels(window_width, sidebar, stats);
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
}
