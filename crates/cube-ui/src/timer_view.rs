//! The central panel: scramble, digits and the finished time.

use std::time::{Duration, Instant};

use cube_core::bests::{self, BestScope};
use cube_core::format::format_ms;
use cube_core::panels::fit_font_px;
use cube_core::{Action, ManualEntry, ManualError, Modifier, Readiness, TimerPhase};
use cube_locale::Strings;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CubeApp, LastResult};
use crate::fonts;
use crate::palette::Palette;
use crate::scroll_fade::FadeScroll;

/// Lines the scramble row always reserves: enough for most events, so that the
/// digits stay put from one scramble to the next.
const SCRAMBLE_MIN_LINES: f32 = 2.;

/// Lines the scramble may take before it scrolls, so that the long scrambles of
/// the big events leave room for the digits in a small window.
const SCRAMBLE_MAX_LINES: f32 = 8.;

/// The share of the window's height the digits may take.
const DIGITS_HEIGHT_SHARE: f32 = 0.3;

/// The padding on each side of the timer panel.
const PANEL_PADDING: Rems = Rems(1.5);

/// What the title bar and window frame take from the window's height, which
/// the panel's room is worked out without. A little too much is safe.
const TITLE_BAR_ALLOWANCE: Rems = Rems(2.5);

/// The room kept beside the digits for the manual mode label and the hint.
const HINT_ALLOWANCE: Rems = Rems(2.);

/// How much an all-time best grows the digits at the peak of a flash.
const ALL_TIME_GROWTH: f32 = 0.06;

/// What the digits show at a given moment.
struct Display {
    text: String,
    color: Hsla,
    hint: Option<String>,
}

impl CubeApp {
    pub(crate) fn render_timer(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let display = self.display(cx);
        let attempt = self.timer.is_active();
        let theme = cx.theme();
        let mono = fonts::timer_digits(cx).unwrap_or_else(|| theme.mono_font_family.clone());
        let muted = theme.muted_foreground;
        // `ring` is the accent in every theme; `accent` is a hover fill in ours.
        let accent = theme.ring;
        let digits = self.fit_digits(&display.text, &mono, attempt, window, cx);

        let room = self.room_beside_digits(digits, window);

        v_flex()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .items_center()
            .p(PANEL_PADDING)
            // The digits sit on their own layer, centered in the panel, so
            // nothing above or below them can move them. The layer comes first
            // so the scramble, tags and preview stay clickable on top of it.
            .child(
                div()
                    .id("timer-pad")
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    // Holding and releasing the digits works like the timer key,
                    // for touchscreens and for a hand resting on the mouse,
                    // while `touch_to_start` is on.
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_pointer_down))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_pointer_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_pointer_up))
                    .child(
                        div()
                            .relative()
                            .flex_none()
                            .when(self.manual_mode() && !attempt, |anchor| {
                                anchor.child(
                                    div()
                                        .absolute()
                                        .bottom_full()
                                        .left_0()
                                        .right_0()
                                        .pb_3()
                                        .flex()
                                        .justify_center()
                                        .whitespace_nowrap()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(muted)
                                        .child(self.strings().manual_mode),
                                )
                            })
                            .child(
                                self.celebrated(
                                    div()
                                        .font_family(mono)
                                        .text_size(digits)
                                        .line_height(digits)
                                        .flex_none()
                                        .whitespace_nowrap()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(display.color)
                                        .child(display.text),
                                    digits,
                                    display.color,
                                    accent,
                                ),
                            )
                            .when_some(display.hint, |anchor, hint| {
                                anchor.child(
                                    div()
                                        .absolute()
                                        .top_full()
                                        .left_0()
                                        .right_0()
                                        .pt_3()
                                        .flex()
                                        .justify_center()
                                        .whitespace_nowrap()
                                        .text_sm()
                                        .text_color(muted)
                                        .child(hint),
                                )
                            }),
                    ),
            )
            .when(!attempt, |panel| {
                // The case takes the place of the preview once it is revealed.
                let preview = if self.trainer_revealed() {
                    None
                } else {
                    self.scramble_preview(room, window)
                };
                let revealed = self.render_revealed_case();
                panel
                    .child(self.render_scramble_bar(room, cx))
                    // Pushes the tags and the preview to the bottom edge.
                    .child(div().flex_1())
                    .children(revealed)
                    .children(preview)
            })
    }

    /// The height between the panel's edge and the digits that the scramble,
    /// or the tags and the preview, may take without reaching the digits. It
    /// leaves out the panel's padding and the room the hints need.
    fn room_beside_digits(&self, digits: Pixels, window: &Window) -> Pixels {
        let rem = window.rem_size();
        let panel = window.viewport_size().height - TITLE_BAR_ALLOWANCE.to_pixels(rem);
        let digits = digits * (1. + ALL_TIME_GROWTH);
        let room =
            (panel - digits) / 2. - PANEL_PADDING.to_pixels(rem) - HINT_ALLOWANCE.to_pixels(rem);
        room.max(px(0.))
    }

    /// Plays the current celebration on the digits. It only restyles them, with
    /// the line height left alone so nothing around them moves, and it ends as
    /// soon as the next attempt begins.
    fn celebrated(&self, digits: Div, size: Pixels, color: Hsla, accent: Hsla) -> AnyElement {
        let showing_result =
            !self.timer.is_active() && self.last_result.is_some() && !self.drilling();
        let celebration = self.celebration.filter(|celebration| {
            let length = Duration::from_millis(celebration.scope.effect_ms());
            showing_result && celebration.started.elapsed() < length
        });
        let Some(celebration) = celebration else {
            return digits.into_any_element();
        };
        let scope = celebration.scope;
        let reduced = self.settings.reduce_motion;
        digits
            .with_animation(
                ("celebration", celebration.solve),
                Animation::new(Duration::from_millis(scope.effect_ms())),
                move |digits, progress| {
                    if reduced {
                        return if progress < 1. {
                            digits.text_color(accent)
                        } else {
                            digits
                        };
                    }
                    let glow = bests::glow(progress, scope.flashes());
                    let digits = digits.text_color(color.blend(accent.opacity(glow)));
                    if scope == BestScope::AllTime {
                        digits.text_size(size * (1. + ALL_TIME_GROWTH * glow))
                    } else {
                        digits
                    }
                },
            )
            .into_any_element()
    }

    /// The size of the digits: the chosen one, unless `text` would not fit the
    /// timer's panel on one line at that size. A time never folds.
    fn fit_digits(
        &self,
        text: &str,
        family: &SharedString,
        attempt: bool,
        window: &Window,
        cx: &App,
    ) -> Pixels {
        let chosen = self.settings.timer_digits_px(self.shown_event());
        let viewport = window.viewport_size();
        let padding = PANEL_PADDING.to_pixels(window.rem_size()) * 2.;
        let panels = if attempt {
            px(0.)
        } else {
            let (sidebar, stats) = self.panel_widths(window, cx);
            if self.drilling() {
                sidebar
            } else {
                sidebar + stats
            }
        };
        let mut digits = font(family.clone());
        digits.weight = FontWeight::MEDIUM;
        let run = TextRun {
            len: text.len(),
            font: digits,
            color: black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shaped =
            window
                .text_system()
                .shape_line(text.to_owned().into(), px(chosen), &[run], None);
        px(fit_font_px(
            chosen,
            f32::from(shaped.width),
            f32::from(viewport.width - panels - padding),
            f32::from(viewport.height) * DIGITS_HEIGHT_SHARE,
        ))
    }

    /// The drawing of the current scramble, unless it is hidden or not ready.
    fn scramble_preview(&self, room: Pixels, window: &Window) -> Option<Div> {
        if !self.settings.scramble_preview {
            return None;
        }
        let event = self.shown_event();
        let scramble = self.shown_scramble()?;
        let image = self.preview.get(event, scramble)?;
        let size = self.settings.preview_size(event);
        // Never taller than the room above the bottom edge once the gap above
        // the drawing is taken out.
        let tallest = (room - px(12.)).max(px(0.));
        let height = (window.viewport_size().height * size.height_share()).min(tallest);
        Some(
            div()
                .flex_none()
                .max_w_full()
                .pt_3()
                .child(image.element(height, size.max_scale())),
        )
    }

    /// Works out the digits, their color and the hint for the timer's phase.
    fn display(&self, cx: &App) -> Display {
        let now = Instant::now();
        let strings = self.strings();
        let timer_key = self
            .keybinds()
            .label(Action::Timer)
            .unwrap_or_else(|| strings.unassigned.to_owned());
        let colors = Palette::of(cx);
        let foreground = cx.theme().foreground;
        let held = match self.timer.readiness(now) {
            Readiness::Released => None,
            Readiness::Waiting => Some((colors.red, strings.hint_hold)),
            Readiness::Ready => Some((colors.green, strings.hint_release)),
        };

        match self.timer.phase() {
            TimerPhase::Idle => {
                let text = self.last_result.map_or_else(|| format_ms(0), result_text);
                let color = match self.last_result.map(|result| result.modifier) {
                    Some(Modifier::DNF) => colors.red,
                    Some(Modifier::PlusTwo) => colors.yellow,
                    _ => foreground,
                };
                if let Some(entry) = self.manual.as_ref().filter(|_| self.manual_mode()) {
                    return self.manual_display(entry, text, color, [foreground, colors.red]);
                }
                let split = self
                    .last_result
                    .and_then(|result| memo_split_hint(strings, result));
                let hint = if split.is_some() {
                    split
                } else if self.hardware_timing() {
                    None
                } else {
                    self.settings
                        .shows_start_hint()
                        .then(|| strings.hint_start.replace("{key}", &timer_key))
                };
                Display { text, color, hint }
            }
            TimerPhase::Armed { ready } => Display {
                text: format_ms(0),
                color: if ready { colors.green } else { colors.red },
                hint: None,
            },
            TimerPhase::Holding { .. } => Display {
                text: format_ms(0),
                color: held.map_or(foreground, |(color, _)| color),
                hint: held.map(|(_, hint)| hint.replace("{key}", &timer_key)),
            },
            TimerPhase::Inspecting { .. } => {
                let elapsed = self.timer.elapsed(now);
                let modifier = self.timer.inspection_modifier(elapsed);
                let text = match modifier {
                    Modifier::None => elapsed.as_secs().to_string(),
                    Modifier::PlusTwo => "+2".to_owned(),
                    Modifier::DNF => "DNF".to_owned(),
                };
                let urgent = modifier != Modifier::None;
                let (color, hint) = held.unwrap_or((
                    if urgent { colors.yellow } else { colors.orange },
                    strings.hint_inspecting,
                ));
                Display {
                    text,
                    color,
                    hint: Some(hint.replace("{key}", &timer_key)),
                }
            }
            TimerPhase::Running { memo, .. } => {
                // A blindfolded attempt is stopped in two presses: the first
                // ends the memo, and the memo stays on screen after it.
                let hint = match memo {
                    Some(memo) if self.timer.blindfolded => strings
                        .hint_memo_stop
                        .replace("{memo}", &format_ms(memo.as_millis() as u64))
                        .replace("{key}", &timer_key),
                    None if self.timer.blindfolded => {
                        strings.hint_memo.replace("{key}", &timer_key)
                    }
                    _ => strings.hint_stop.replace("{key}", &timer_key),
                };
                Display {
                    text: format_ms(self.timer.elapsed(now).as_millis() as u64),
                    color: foreground,
                    hint: (!self.hardware_timing()).then_some(hint),
                }
            }
        }
    }

    /// The digits and hint while a time is being typed in manual mode: the
    /// typed text, or the last result until typing starts.
    fn manual_display(
        &self,
        entry: &ManualEntry,
        last: String,
        last_color: Hsla,
        [foreground, error_color]: [Hsla; 2],
    ) -> Display {
        let strings = self.strings();
        let (text, color) = if entry.is_empty() {
            (last, last_color)
        } else {
            (entry.text().to_owned(), foreground)
        };
        let (color, hint) = match entry.error() {
            Some(ManualError::Empty) => (color, Some(strings.manual_empty)),
            Some(ManualError::Invalid) => (error_color, Some(strings.manual_invalid)),
            None => (color, None),
        };
        Display {
            text,
            color,
            hint: hint.map(str::to_owned),
        }
    }

    fn render_scramble_bar(
        &self,
        room: Pixels,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strings = self.strings();
        let event = self.shown_event();
        let scramble = self.shown_scramble().map(str::to_owned);
        let font_size = px(self.settings.scramble_font_px(event));
        let line = px(self.settings.scramble_line_px(event));
        // The room above the digits, less the gap under the scramble.
        let tallest = (room - px(12.)).max(px(0.));
        let theme = cx.theme();
        let mono = theme.mono_font_family.clone();
        let muted = theme.muted_foreground;
        let scramble_color = Palette::of(cx).scramble;
        // Blank while a scramble is prepared, so a fast one never flashes text.
        // A no-break space keeps the line's height.
        let text: SharedString = match &scramble {
            Some(scramble) => cube_core::scramble::display_text(event, scramble).into(),
            None if self.generating_visible => strings.generating_scramble.into(),
            None => "\u{a0}".into(),
        };
        // A new scramble is a new key, so the scroll area starts from its top.
        let scroll_id = text.clone();

        h_flex()
            .flex_none()
            .w_full()
            .min_h((line * SCRAMBLE_MIN_LINES).min(tallest))
            .pb_3()
            .child(
                FadeScroll::new(scroll_id, theme.background)
                    .flex_1()
                    .min_w_0()
                    .h_auto()
                    .max_h((line * SCRAMBLE_MAX_LINES).min(tallest))
                    .text_center()
                    .font_family(mono)
                    .text_size(font_size)
                    .line_height(line)
                    .when(scramble.is_none(), |text| text.text_color(muted))
                    .when(scramble.is_some(), |text| {
                        text.text_color(scramble_color).cursor_pointer().tooltip(
                            move |window, cx| Tooltip::new(strings.copy_scramble).build(window, cx),
                        )
                    })
                    .on_click(cx.listener(|app, _, window, cx| {
                        if let Some(scramble) = app.shown_scramble() {
                            let scramble = scramble.to_owned();
                            cx.write_to_clipboard(ClipboardItem::new_string(scramble));
                            app.toast(app.strings().scramble_copied, window, cx);
                        }
                        app.focus.focus(window, cx);
                    }))
                    .child(text),
            )
    }
}

/// The memo and execution of a finished blindfolded attempt, if it had a memo.
pub(crate) fn memo_split_hint(strings: &'static Strings, result: LastResult) -> Option<String> {
    let memo = result.memo_ms?.min(result.time_ms);
    Some(
        strings
            .result_memo_split
            .replace("{memo}", &format_ms(memo))
            .replace("{execution}", &format_ms(result.time_ms - memo)),
    )
}

/// Formats a finished attempt the way it will be recorded.
pub(crate) fn result_text(result: LastResult) -> String {
    match result.modifier {
        Modifier::None => format_ms(result.time_ms),
        Modifier::PlusTwo => format!("{}+", format_ms(result.time_ms + 2_000)),
        Modifier::DNF => "DNF".to_owned(),
    }
}
