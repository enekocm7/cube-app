//! The central panel: event, scramble, digits and the latest result's actions.

use std::time::Instant;

use cube_core::format::format_ms;
use cube_core::{Modifier, Readiness, TimerPhase, WcaEvent};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CubeApp, LastResult};
use crate::palette::Palette;

/// Digit size of the timer.
const DIGITS_SIZE: Pixels = px(112.);

/// What the digits show at a given moment.
struct Display {
    text: String,
    color: Hsla,
    hint: &'static str,
}

impl CubeApp {
    pub(crate) fn render_timer(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let display = self.display(cx);
        let attempt = self.timer.is_active();
        let theme = cx.theme();
        let mono = theme.mono_font_family.clone();
        let muted = theme.muted_foreground;

        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .items_center()
            .p_6()
            .when(!attempt, |panel| panel.child(self.render_scramble_bar(cx)))
            .child(
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(
                        div()
                            .font_family(mono)
                            .text_size(DIGITS_SIZE)
                            .line_height(DIGITS_SIZE)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(display.color)
                            .child(display.text),
                    )
                    .child(div().text_sm().text_color(muted).child(display.hint))
                    .when_some(self.last_result.filter(|_| !attempt), |column, result| {
                        column.child(self.render_result_actions(result, cx))
                    }),
            )
    }

    /// Works out the digits, their color and the hint for the timer's phase.
    fn display(&self, cx: &App) -> Display {
        let now = Instant::now();
        let strings = self.strings();
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
                Display {
                    text,
                    color,
                    hint: strings.hint_start,
                }
            }
            TimerPhase::Holding { .. } => {
                let (color, hint) = held.unwrap_or((foreground, strings.hint_start));
                Display {
                    text: format_ms(0),
                    color,
                    hint,
                }
            }
            TimerPhase::Inspecting { .. } => {
                let elapsed = self.timer.elapsed(now);
                let remaining = self.timer.inspection_limit.saturating_sub(elapsed);
                let text = match self.timer.inspection_modifier(elapsed) {
                    Modifier::None => remaining.as_secs_f32().ceil().to_string(),
                    Modifier::PlusTwo => "+2".to_owned(),
                    Modifier::DNF => "DNF".to_owned(),
                };
                let urgent = self.timer.inspection_modifier(elapsed) != Modifier::None;
                let (color, hint) = held.unwrap_or((
                    if urgent { colors.yellow } else { colors.orange },
                    strings.hint_inspecting,
                ));
                Display { text, color, hint }
            }
            TimerPhase::Running { .. } => Display {
                text: format_ms(self.timer.elapsed(now).as_millis() as u64),
                color: foreground,
                hint: strings.hint_stop,
            },
        }
    }

    fn render_scramble_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings();
        let entity = cx.entity().downgrade();
        let current = self.event;
        let scramble = self.scramble.clone();
        let theme = cx.theme();
        let mono = theme.mono_font_family.clone();
        let muted = theme.muted_foreground;
        let text: SharedString = scramble
            .clone()
            .unwrap_or_else(|| strings.generating_scramble.to_owned())
            .into();

        v_flex()
            .w_full()
            .gap_3()
            .items_center()
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("event")
                            .outline()
                            .small()
                            .label(current.name())
                            .tab_stop(false)
                            .dropdown_menu(move |mut menu, _, _| {
                                for event in WcaEvent::ALL {
                                    let entity = entity.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(event.name())
                                            .checked(event == current)
                                            .on_click(move |_, _, cx| {
                                                entity
                                                    .update(cx, |app, cx| app.set_event(event, cx))
                                                    .ok();
                                            }),
                                    );
                                }
                                menu
                            }),
                    )
                    .child(
                        Button::new("next-scramble")
                            .ghost()
                            .small()
                            .icon(IconName::Redo)
                            .tooltip(strings.new_scramble)
                            .tab_stop(false)
                            .on_click(cx.listener(|app, _, window, cx| {
                                app.skip_scramble(cx);
                                app.focus.focus(window, cx);
                            })),
                    )
                    .child(
                        Button::new("copy-scramble")
                            .ghost()
                            .small()
                            .icon(IconName::Copy)
                            .tooltip(strings.copy_scramble)
                            .tab_stop(false)
                            .on_click(cx.listener(move |app, _, window, cx| {
                                if let Some(scramble) = &app.scramble {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        scramble.clone(),
                                    ));
                                    app.toast(app.strings().scramble_copied, window, cx);
                                }
                                app.focus.focus(window, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .max_w(px(760.))
                    .text_center()
                    .font_family(mono)
                    .text_lg()
                    .when(scramble.is_none(), |text| text.text_color(muted))
                    .child(text),
            )
    }

    /// Penalty shortcuts for the attempt that just finished.
    fn render_result_actions(
        &self,
        result: LastResult,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strings = self.strings();
        let index = self.session().len().saturating_sub(1);
        let penalty =
            |id: &'static str, label: &'static str, modifier: Modifier, cx: &mut Context<Self>| {
                Button::new(id)
                    .outline()
                    .small()
                    .label(label)
                    .tab_stop(false)
                    .selected(result.modifier == modifier)
                    .on_click(cx.listener(move |app, _, window, cx| {
                        app.toggle_penalty(index, modifier, window, cx);
                        app.focus.focus(window, cx);
                    }))
            };
        h_flex()
            .gap_2()
            .child(penalty("last-plus-two", "+2", Modifier::PlusTwo, cx))
            .child(penalty("last-dnf", "DNF", Modifier::DNF, cx))
            .child(
                Button::new("last-delete")
                    .ghost()
                    .small()
                    .label(strings.delete)
                    .tab_stop(false)
                    .on_click(cx.listener(move |app, _, window, cx| {
                        app.confirm_delete_solve(index, window, cx);
                    })),
            )
    }
}

/// Formats a finished attempt the way it will be recorded.
fn result_text(result: LastResult) -> String {
    match result.modifier {
        Modifier::None => format_ms(result.time_ms),
        Modifier::PlusTwo => format!("{}+", format_ms(result.time_ms + 2_000)),
        Modifier::DNF => "DNF".to_owned(),
    }
}
