//! Where the main window was last, so it can open there again.

use serde::{Deserialize, Serialize};

/// A rectangle in logical pixels, such as a monitor's area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Area {
    const fn contains(self, x: i64, y: i64) -> bool {
        x >= self.x as i64
            && y >= self.y as i64
            && x < self.x as i64 + self.width as i64
            && y < self.y as i64 + self.height as i64
    }
}

/// The window's position and size, in logical pixels, and whether it was
/// maximized. When maximized, the bounds are the size to return to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub maximized: bool,
}

impl WindowState {
    /// Returns this state if it can be restored on one of `displays`: the
    /// window's center must lie on a connected monitor, so a window saved on a
    /// monitor that is gone falls back to the default. The size is limited to
    /// that monitor and to at least `min`. `None` means open at the default.
    #[must_use]
    pub fn fit(self, displays: &[Area], min: (u32, u32)) -> Option<Self> {
        if self.width == 0 || self.height == 0 {
            return None;
        }
        let center_x = i64::from(self.x) + i64::from(self.width / 2);
        let center_y = i64::from(self.y) + i64::from(self.height / 2);
        let display = displays
            .iter()
            .find(|display| display.contains(center_x, center_y))?;
        let width = self.width.clamp(min.0.min(display.width), display.width);
        let height = self.height.clamp(min.1.min(display.height), display.height);
        // Keep the whole window on the monitor it is centered on.
        let x = i64::from(self.x).clamp(
            i64::from(display.x),
            i64::from(display.x) + i64::from(display.width - width),
        );
        let y = i64::from(self.y).clamp(
            i64::from(display.y),
            i64::from(display.y) + i64::from(display.height - height),
        );
        Some(Self {
            x: i32::try_from(x).ok()?,
            y: i32::try_from(y).ok()?,
            width,
            height,
            maximized: self.maximized,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: (u32, u32) = (910, 560);
    const MAIN: Area = Area {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    const SECOND: Area = Area {
        x: 1920,
        y: 0,
        width: 1280,
        height: 1024,
    };

    fn state(x: i32, y: i32, width: u32, height: u32) -> WindowState {
        WindowState {
            x,
            y,
            width,
            height,
            maximized: false,
        }
    }

    #[test]
    fn a_window_on_a_connected_monitor_is_restored_as_saved() {
        let saved = state(100, 80, 1180, 760);
        assert_eq!(saved.fit(&[MAIN], MIN), Some(saved));
        let on_second = state(2000, 50, 1000, 700);
        assert_eq!(on_second.fit(&[MAIN, SECOND], MIN), Some(on_second));
    }

    #[test]
    fn a_window_on_a_missing_monitor_falls_back_to_the_default() {
        assert_eq!(state(2000, 50, 1000, 700).fit(&[MAIN], MIN), None);
        assert_eq!(state(-3000, 0, 1000, 700).fit(&[MAIN], MIN), None);
        assert_eq!(state(0, 0, 1000, 700).fit(&[], MIN), None);
    }

    #[test]
    fn an_oversized_or_overhanging_window_is_pulled_onto_the_monitor() {
        let fitted = state(1000, 600, 1180, 760).fit(&[MAIN], MIN);
        let fitted = fitted.unwrap();
        assert_eq!((fitted.x, fitted.y), (740, 320));

        let huge = state(-1000, -1000, 5000, 4000).fit(&[MAIN], MIN).unwrap();
        assert_eq!((huge.width, huge.height), (1920, 1080));
    }

    #[test]
    fn a_tiny_window_is_raised_to_the_minimum_size() {
        let fitted = state(10, 10, 100, 100).fit(&[MAIN], MIN).unwrap();
        assert_eq!((fitted.width, fitted.height), MIN);
    }

    #[test]
    fn an_empty_window_is_ignored() {
        assert_eq!(state(10, 10, 0, 500).fit(&[MAIN], MIN), None);
    }

    #[test]
    fn the_maximized_flag_is_kept() {
        let saved = WindowState {
            maximized: true,
            ..state(100, 80, 1180, 760)
        };
        assert!(saved.fit(&[MAIN], MIN).unwrap().maximized);
    }
}
