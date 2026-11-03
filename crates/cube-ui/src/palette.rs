//! The color system.
//!
//! The sticker colors of a cube carry meaning here, the way a stackmat's lights
//! do: red while the timer is held, green when it is ready, orange during
//! inspection, yellow for a `+2`.

use cube_core::settings::ThemePreference;
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;

/// The colors used directly by cube's own views.
#[derive(Clone, Copy)]
pub struct Palette {
    pub red: Hsla,
    pub green: Hsla,
    pub orange: Hsla,
    pub yellow: Hsla,
    pub blue: Hsla,
}

impl Palette {
    /// Returns the sticker colors tuned for contrast on the current background.
    pub fn of(cx: &App) -> Self {
        if Theme::global(cx).mode.is_dark() {
            Self {
                red: rgb(0xff5a5f).into(),
                green: rgb(0x2ed47a).into(),
                orange: rgb(0xff9a3d).into(),
                yellow: rgb(0xffd93b).into(),
                blue: rgb(0x5b9bff).into(),
            }
        } else {
            Self {
                red: rgb(0xc81e24).into(),
                green: rgb(0x0a7f46).into(),
                orange: rgb(0xb45309).into(),
                yellow: rgb(0x8a6a00).into(),
                blue: rgb(0x1d5fd1).into(),
            }
        }
    }
}

/// Assigns one color to both the theme's color table and its derived tokens,
/// because components read either.
macro_rules! set_color {
    ($theme:expr, $($field:ident = $value:expr),+ $(,)?) => {
        $({
            let color: Hsla = rgb($value).into();
            $theme.colors.$field = color;
            $theme.tokens.$field = color.into();
        })+
    };
}

/// Switches to the preferred mode and applies cube's colors on top of it.
pub fn apply(preference: ThemePreference, cx: &mut App) {
    let mode = match preference {
        ThemePreference::System => ThemeMode::from(cx.window_appearance()),
        ThemePreference::Light => ThemeMode::Light,
        ThemePreference::Dark => ThemeMode::Dark,
    };
    Theme::change(mode, None, cx);

    let theme = Theme::global_mut(cx);
    if mode.is_dark() {
        set_color!(
            theme,
            background = 0x141a2e,
            foreground = 0xe9edf7,
            muted = 0x1c2440,
            muted_foreground = 0x9aa7c7,
            border = 0x2c3658,
            popover = 0x1c2440,
            popover_foreground = 0xe9edf7,
            title_bar = 0x141a2e,
            title_bar_border = 0x2c3658,
            primary = 0x2f7bff,
            primary_hover = 0x5b9bff,
            primary_active = 0x1d5fd1,
            primary_foreground = 0xffffff,
            button_primary = 0x2f6fe6,
            button_primary_hover = 0x4a86f0,
            button_primary_active = 0x1d5fd1,
            button_primary_foreground = 0xffffff,
            ring = 0x5b9bff,
        );
    } else {
        set_color!(
            theme,
            background = 0xf2f4fa,
            foreground = 0x141a2e,
            muted = 0xe4e8f3,
            muted_foreground = 0x4d5875,
            border = 0xcdd3e4,
            popover = 0xffffff,
            popover_foreground = 0x141a2e,
            title_bar = 0xf2f4fa,
            title_bar_border = 0xcdd3e4,
            primary = 0x1d5fd1,
            primary_hover = 0x2f6fe6,
            primary_active = 0x174ba6,
            primary_foreground = 0xffffff,
            button_primary = 0x1d5fd1,
            button_primary_hover = 0x2f6fe6,
            button_primary_active = 0x174ba6,
            button_primary_foreground = 0xffffff,
            ring = 0x1d5fd1,
        );
    }
}
