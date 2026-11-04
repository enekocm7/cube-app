//! The color system.
//!
//! The sticker colors of a cube carry meaning here, the way a stackmat's lights
//! do: red while the timer is held, green when it is ready, orange during
//! inspection, yellow for a `+2`.
//!
//! cube-tui theme files only describe a handful of colors, so they are mapped
//! onto the closest gpui-kit tokens and the rest are derived from them.

use cube_core::Settings;
use cube_core::persistence;
use cube_core::settings::ThemePreference;
use cube_core::theme::{ThemeColor, ThemeColors};
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
    /// The scramble text, which cube-tui themes color separately.
    pub scramble: Hsla,
}

/// The scramble color of the active theme, kept outside [`Theme`] because
/// gpui-kit has no scramble token.
#[derive(Clone, Copy)]
struct ScrambleColor(Hsla);

impl Global for ScrambleColor {}

impl Palette {
    /// Returns the sticker colors tuned for contrast on the current background.
    pub fn of(cx: &App) -> Self {
        let theme = Theme::global(cx);
        let scramble = cx
            .try_global::<ScrambleColor>()
            .map_or(theme.foreground, |color| color.0);
        if theme.mode.is_dark() {
            Self {
                red: rgb(0xff5a5f).into(),
                green: rgb(0x2ed47a).into(),
                orange: rgb(0xff9a3d).into(),
                yellow: rgb(0xffd93b).into(),
                blue: rgb(0x5b9bff).into(),
                scramble,
            }
        } else {
            Self {
                red: rgb(0xc81e24).into(),
                green: rgb(0x0a7f46).into(),
                orange: rgb(0xb45309).into(),
                yellow: rgb(0x8a6a00).into(),
                blue: rgb(0x1d5fd1).into(),
                scramble,
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

/// Like [`set_color`], for colors that are already [`Hsla`].
macro_rules! set_hsla {
    ($theme:expr, $($field:ident = $value:expr),+ $(,)?) => {
        $({
            let color: Hsla = $value;
            $theme.colors.$field = color;
            $theme.tokens.$field = color.into();
        })+
    };
}

/// Stacks `over` on `base` at `amount`, returning an opaque color.
fn mix(base: Hsla, over: Hsla, amount: f32) -> Hsla {
    base.blend(over.opacity(amount))
}

fn hsla(color: ThemeColor) -> Hsla {
    rgb(color.rgb()).into()
}

/// Applies the built-in or the selected cube-tui theme.
pub fn apply(settings: &Settings, cx: &mut App) {
    let custom = if settings.theme_file.trim().is_empty() {
        None
    } else {
        persistence::load_theme(&settings.theme_file).ok()
    };
    let mode = match &custom {
        Some(colors) if colors.background.is_dark() => ThemeMode::Dark,
        Some(_) => ThemeMode::Light,
        None => match settings.theme {
            ThemePreference::System => ThemeMode::from(cx.window_appearance()),
            ThemePreference::Light => ThemeMode::Light,
            ThemePreference::Dark => ThemeMode::Dark,
        },
    };
    Theme::change(mode, None, cx);

    let scramble = {
        let theme = Theme::global_mut(cx);
        if let Some(colors) = &custom {
            apply_custom(theme, colors);
            hsla(colors.scramble)
        } else {
            apply_builtin(theme, mode);
            theme.colors.foreground
        }
    };
    cx.set_global(ScrambleColor(scramble));
}

/// Switches to the preferred mode and applies cube's built-in colors.
fn apply_builtin(theme: &mut Theme, mode: ThemeMode) {
    if mode.is_dark() {
        set_color!(
            theme,
            background = 0x0a0a0a,
            foreground = 0xededed,
            muted = 0x161616,
            muted_foreground = 0x8c8c8c,
            border = 0x232323,
            popover = 0x121212,
            popover_foreground = 0xededed,
            title_bar = 0x0a0a0a,
            title_bar_border = 0x232323,
            primary = 0x3b82f6,
            primary_hover = 0x60a5fa,
            primary_active = 0x2563eb,
            primary_foreground = 0xffffff,
            button_primary = 0x3b82f6,
            button_primary_hover = 0x60a5fa,
            button_primary_active = 0x2563eb,
            button_primary_foreground = 0xffffff,
            ring = 0x60a5fa,
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

/// Maps a cube-tui theme's seven colors onto gpui-kit's tokens.
fn apply_custom(theme: &mut Theme, colors: &ThemeColors) {
    let background = hsla(colors.background);
    let text = hsla(colors.text);
    let border = hsla(colors.border);
    let selection = hsla(colors.selection);
    let selection_text = hsla(colors.selection_text);
    let accent = hsla(colors.accent);
    let muted = mix(background, text, 0.08);
    let muted_foreground = mix(text, background, 0.4);
    let hover = mix(background, text, 0.16);
    let button = mix(background, text, 0.06);
    let active = mix(background, text, 0.24);
    let selection_hover = mix(selection, text, 0.16);
    let selection_active = mix(selection, background, 0.16);
    let popover = mix(background, text, 0.04);
    let scrollbar = mix(background, border, 0.6);

    set_hsla!(
        theme,
        background = background,
        title_bar = background,
        sidebar = background,
        list = background,
        table = background,
        popover = popover,
        foreground = text,
        popover_foreground = text,
        sidebar_foreground = text,
        caret = text,
        muted = muted,
        muted_foreground = muted_foreground,
        border = border,
        title_bar_border = border,
        sidebar_border = border,
        table_row_border = border,
        list_active_border = border,
        input = border,
        window_border = border,
        scrollbar = scrollbar,
        scrollbar_thumb = border,
        scrollbar_thumb_hover = text,
        primary = selection,
        primary_hover = selection_hover,
        primary_active = selection_active,
        primary_foreground = selection_text,
        button_primary = selection,
        button_primary_hover = selection_hover,
        button_primary_active = selection_active,
        button_primary_foreground = selection_text,
        selection = selection,
        list_active = selection,
        sidebar_accent = selection,
        sidebar_primary = selection,
        sidebar_primary_foreground = selection_text,
        secondary = muted,
        secondary_foreground = text,
        secondary_hover = hover,
        secondary_active = active,
        button = button,
        button_foreground = text,
        button_hover = hover,
        button_active = active,
        group_box = muted,
        group_box_foreground = text,
        list_hover = hover,
        list_even = muted,
        list_head = muted,
        table_hover = hover,
        table_even = muted,
        table_head = muted,
        accent = accent,
        accent_foreground = text,
        ring = accent,
        chart_1 = accent,
        chart_2 = selection,
        chart_3 = text,
        switch = selection,
        slider_bar = selection,
        slider_thumb = selection_text,
    );
}
