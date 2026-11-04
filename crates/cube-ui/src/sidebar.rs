//! The left panel: the session and event pickers and the session's solves,
//! newest first.

use std::ops::Range;

use cube_core::{Modifier, WcaEvent};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::{Scrollbar, ScrollbarMode};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CubeApp, average_label, format_average_at};
use crate::dialog_size::{DialogSize, dialog_title, dialog_width};
use crate::palette::Palette;
use crate::scroll_fade::fading_scroll;

/// The widest the session picker gets, however long the session's name is.
const SESSION_MAX_WIDTH: Pixels = px(120.);

/// The widest a session's name gets in the session menu.
const SESSION_MENU_MAX_WIDTH: Pixels = px(260.);

const AVERAGE_WIDTH: Pixels = px(64.);

/// Extra right padding while the average column is shown, so the averages
/// clear the overlay scrollbar instead of touching it.
const AVERAGE_SCROLLBAR_GUTTER: Pixels = px(10.);

/// The rolling average the solve list can show.
const LIST_AVERAGE_SIZE: usize = 5;

impl CubeApp {
    pub(crate) fn render_sidebar(
        &self,
        width: Pixels,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strings = self.strings();
        let count = self.session().len();
        let theme = cx.theme();
        let (border, muted) = (theme.border, theme.muted_foreground);
        let background = theme.background;
        let show_average = self.settings.solve_list_ao5 && count > 0;

        v_flex()
            .w(width)
            .flex_shrink_0()
            .h_full()
            .border_r_1()
            .border_color(border)
            .child(self.render_session_picker(cx))
            .child(
                h_flex()
                    .px_3()
                    .py_2()
                    .when(show_average, |header| {
                        header.pr(px(12.) + AVERAGE_SCROLLBAR_GUTTER)
                    })
                    .justify_between()
                    .border_t_1()
                    .border_color(border)
                    .text_xs()
                    .text_color(muted)
                    .child(strings.solves)
                    .when(show_average, |header| {
                        header.child(
                            div()
                                .w(AVERAGE_WIDTH)
                                .text_right()
                                .child(average_label(LIST_AVERAGE_SIZE)),
                        )
                    }),
            )
            .child(if count == 0 {
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_3()
                    .text_sm()
                    .text_color(muted)
                    .child(strings.no_solves)
                    .into_any_element()
            } else {
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        fading_scroll(
                            uniform_list("solves", count, cx.processor(Self::render_solve_rows))
                                .size_full()
                                .track_scroll(&self.solves_scroll),
                            &self.solves_scroll,
                            background,
                        )
                        .size_full(),
                    )
                    .child(Scrollbar::vertical(&self.solves_scroll).mode(
                        if self.settings.always_show_scrollbar {
                            ScrollbarMode::Always
                        } else {
                            ScrollbarMode::Scrolling
                        },
                    ))
                    .into_any_element()
            })
    }

    fn render_session_picker(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let entity = cx.entity().downgrade();
        let labels: Vec<String> = (0..self.sessions.len())
            .map(|index| self.session_label(index))
            .collect();
        let current = self.current;
        let current_label = labels[current].clone();

        let session = h_flex()
            .flex_1()
            .min_w_0()
            .gap_1()
            // A dropdown menu wraps its button, so sizing has to go on a parent. The
            // name is the only thing here that gives way when space runs out.
            .child(
                v_flex().min_w_0().max_w(SESSION_MAX_WIDTH).child(
                    Button::new("session")
                        .outline()
                        .small()
                        .w_full()
                        .label(current_label)
                        .tab_stop(false)
                        .dropdown_menu(move |mut menu, _, _| {
                            for (index, label) in labels.iter().enumerate() {
                                let entity = entity.clone();
                                let label = label.clone();
                                menu = menu.item(
                                    PopupMenuItem::element(move |_, _| {
                                        div()
                                            .max_w(SESSION_MENU_MAX_WIDTH)
                                            .truncate()
                                            .child(label.clone())
                                    })
                                    .checked(index == current)
                                    .on_click(
                                        move |_, _, cx| {
                                            entity
                                                .update(cx, |app, cx| app.select_session(index, cx))
                                                .ok();
                                        },
                                    ),
                                );
                            }
                            menu
                        }),
                ),
            )
            .child(self.render_session_actions(cx));

        h_flex()
            .p_2()
            .gap_2()
            .justify_between()
            .child(session)
            .child(self.render_event_picker(cx))
    }

    /// The current event and the control to skip its scramble.
    fn render_event_picker(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings();
        let entity = cx.entity().downgrade();
        let event = self.event;

        h_flex()
            .flex_none()
            .gap_1()
            .child(
                div().flex_none().child(
                    Button::new("event")
                        .text()
                        .small()
                        .label(event.name())
                        .dropdown_caret(true)
                        .tab_stop(false)
                        .dropdown_menu(move |mut menu, _, _| {
                            for item in WcaEvent::ALL {
                                let entity = entity.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(item.name())
                                        .checked(item == event)
                                        .on_click(move |_, _, cx| {
                                            entity
                                                .update(cx, |app, cx| app.set_event(item, cx))
                                                .ok();
                                        }),
                                );
                            }
                            menu
                        }),
                ),
            )
            .child(
                Button::new("next-scramble")
                    .ghost()
                    .small()
                    .flex_none()
                    .icon(IconName::Redo)
                    .tooltip(strings.new_scramble)
                    .tab_stop(false)
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.skip_scramble(cx);
                        app.focus.focus(window, cx);
                    })),
            )
    }

    fn render_session_actions(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings();

        h_flex()
            .flex_none()
            .gap_1()
            .child(
                Button::new("session-add")
                    .ghost()
                    .small()
                    .icon(IconName::Plus)
                    .tooltip(strings.new_session)
                    .tab_stop(false)
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.add_session(window, cx);
                        app.focus.focus(window, cx);
                    })),
            )
            .child(
                Button::new("session-rename")
                    .ghost()
                    .small()
                    .icon(IconName::Pencil)
                    .tooltip(strings.rename_session)
                    .tab_stop(false)
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.open_rename_session(window, cx);
                    })),
            )
            .child(
                Button::new("session-delete")
                    .danger()
                    .small()
                    .icon(IconName::Trash)
                    .tooltip(strings.delete_session)
                    .tab_stop(false)
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.confirm_delete_session(window, cx);
                    })),
            )
    }

    /// Row `0` is the newest solve.
    fn render_solve_rows(
        &mut self,
        range: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let colors = Palette::of(cx);
        let theme = cx.theme();
        let (muted, hover, selected_bg) = (theme.muted_foreground, theme.muted, theme.accent);
        let mono = theme.mono_font_family.clone();
        let solves = self.session().solves();
        let total = solves.len();
        let show_average = self.settings.solve_list_ao5;
        let selected_solve = self.selected_solve;

        range
            .filter_map(|row| {
                let index = total.checked_sub(row + 1)?;
                let solve = &solves[index];
                let time_color = match solve.modifier() {
                    Modifier::DNF => Some(colors.red),
                    Modifier::PlusTwo => Some(colors.yellow),
                    Modifier::None if solve.was_single_record() => Some(colors.green),
                    Modifier::None => None,
                };
                Some(
                    h_flex()
                        .id(("solve", index))
                        .w_full()
                        .h(px(28.))
                        .px_3()
                        .when(show_average, |row| {
                            row.pr(px(12.) + AVERAGE_SCROLLBAR_GUTTER)
                        })
                        .gap_2()
                        .text_sm()
                        .font_family(mono.clone())
                        .cursor_pointer()
                        .when(selected_solve == Some(index), |row| row.bg(selected_bg))
                        .hover(move |row| row.bg(hover))
                        .child(
                            div()
                                .w(px(36.))
                                .text_xs()
                                .text_color(muted)
                                .child((index + 1).to_string()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .when_some(time_color, gpui_kit::Styled::text_color)
                                .child(solve.to_string()),
                        )
                        .when(show_average, |row| {
                            row.child(
                                div()
                                    .w(AVERAGE_WIDTH)
                                    .text_right()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(format_average_at(solves, index, LIST_AVERAGE_SIZE)),
                            )
                        })
                        .on_click(cx.listener(move |app, _, window, cx| {
                            app.selected_solve = Some(index);
                            app.open_solve_details(index, window, cx);
                        }))
                        .into_any_element(),
                )
            })
            .collect()
    }

    pub(crate) fn open_rename_session(&self, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.strings();
        let index = self.current;
        let name = self.session().name().to_owned();
        let placeholder = self.session_label(index);
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(name)
                .placeholder(placeholder)
        });
        let entity = cx.entity().downgrade();
        let blur = cx.focus_handle();
        let first_build = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first_build.replace(false) {
                input.focus_handle(cx).focus(window, cx);
            }
            let (input_for_save, entity) = (input.clone(), entity.clone());
            dialog
                .title(dialog_title(strings.rename_session, strings.close))
                .close_button(false)
                .w(dialog_width(DialogSize::COMPACT, window))
                .child(
                    div()
                        .id("rename-body")
                        .track_focus(&blur)
                        .py_2()
                        // Clicking the dialog's empty space moves focus off the
                        // field; presses inside the field stop below.
                        .on_mouse_down(MouseButton::Left, {
                            let blur = blur.clone();
                            move |_, window, cx| blur.focus(window, cx)
                        })
                        .child(
                            div()
                                .id("rename-field")
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .child(Input::new(&input).focus_bordered(false)),
                        ),
                )
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("rename-cancel")
                                .outline()
                                .label(strings.cancel)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("rename-save")
                                .primary()
                                .label(strings.save)
                                .on_click(move |_, window, cx| {
                                    let name = input_for_save.read(cx).value().to_string();
                                    entity
                                        .update(cx, |app, cx| {
                                            app.rename_session(index, &name, window, cx);
                                            app.focus.focus(window, cx);
                                        })
                                        .ok();
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }

    pub(crate) fn confirm_delete_session(&self, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.strings();
        if self.sessions.len() <= 1 {
            self.toast(strings.last_session, window, cx);
            return;
        }
        let index = self.current;
        let message = strings
            .delete_session_confirm
            .replace("{name}", &self.session_label(index))
            .replace("{count}", &self.session().len().to_string());
        let on_confirm = cx.listener(move |app: &mut Self, (): &(), window, cx| {
            app.delete_session(index, window, cx);
            app.focus.focus(window, cx);
        });
        self.open_confirm(strings.delete_session, message, on_confirm, window, cx);
    }

    /// Opens a destructive confirmation; `on_confirm` runs when it is accepted.
    pub(crate) fn open_confirm(
        &self,
        title: &'static str,
        message: String,
        on_confirm: impl Fn(&(), &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let strings = self.strings();
        let on_confirm = std::rc::Rc::new(on_confirm);
        window.open_dialog(cx, move |dialog, window, _| {
            let on_confirm = std::rc::Rc::clone(&on_confirm);
            dialog
                .title(dialog_title(title, strings.close))
                .close_button(false)
                .w(dialog_width(DialogSize::COMPACT, window))
                .child(message.clone())
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("confirm-cancel")
                                .outline()
                                .label(strings.cancel)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("confirm-delete")
                                .danger()
                                .label(strings.delete)
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    on_confirm(&(), window, cx);
                                }),
                        ),
                )
        });
    }
}
