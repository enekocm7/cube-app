//! The bundled typeface for times, used only when the setting is on.
//!
//! Times otherwise use the system's monospace font, which differs from one
//! platform to the next and draws a heavy slashed zero at timer size on
//! Windows. `JetBrains Mono` is embedded so the digits look the same everywhere:
//! it is clean at large sizes, has tabular digits and uses a light dotted zero.
//! It ships under the SIL Open Font License; see `assets/fonts/OFL.txt`.

use std::borrow::Cow;

use gpui_kit::*;

/// The family name inside the bundled font files.
pub(crate) const DIGITS_FAMILY: &str = "JetBrains Mono";

/// Whether the bundled font was registered with this app's text system.
struct DigitsFont(bool);

impl Global for DigitsFont {}

/// The bundled family the timer digits use when it is chosen for the timer
/// alone. `None` leaves them in the theme's monospace family.
struct TimerDigits(Option<SharedString>);

impl Global for TimerDigits {}

/// Registers the bundled font on first use and returns the family to render
/// times with, or `None` when the system font should stay in place.
pub(crate) fn family(cx: &mut App) -> Option<SharedString> {
    if cx.try_global::<DigitsFont>().is_none() {
        let fonts: Vec<Cow<'static, [u8]>> = vec![
            Cow::Borrowed(&include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf")[..]),
            Cow::Borrowed(&include_bytes!("../assets/fonts/JetBrainsMono-Medium.ttf")[..]),
        ];
        let registered = cx.text_system().add_fonts(fonts).inspect_err(|error| {
            eprintln!("cube: could not load the bundled digit font: {error}");
        });
        cx.set_global(DigitsFont(registered.is_ok()));
    }
    cx.try_global::<DigitsFont>()
        .is_some_and(|font| font.0)
        .then(|| DIGITS_FAMILY.into())
}

/// Remembers which family the timer's digits use, if any.
pub(crate) fn set_timer_digits(cx: &mut App, family: Option<SharedString>) {
    cx.set_global(TimerDigits(family));
}

/// The family the timer's digits use, or `None` for the theme's monospace.
pub(crate) fn timer_digits(cx: &App) -> Option<SharedString> {
    cx.try_global::<TimerDigits>()
        .and_then(|font| font.0.clone())
}
