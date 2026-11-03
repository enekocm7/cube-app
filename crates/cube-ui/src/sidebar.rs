//! The left panel: the session picker and the session's solves, newest first.

use std::ops::Range;

use cube_core::Modifier;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CubeApp, format_average_at};
use crate::palette::Palette;

const SIDEBAR_WIDTH: Pixels = px(250.);

impl CubeApp {
    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings();
        let count = self.session().len();
        let theme = cx.theme();
        let (border, muted) = (theme.border, theme.muted_foreground);

        v_flex()
            .w(SIDEBAR_WIDTH)
            .flex_shrink_0()
            .h_full()
            .border_r_1()
            .border_color(border)
            .child(self.render_session_picker(cx))
            .child(
                h_flex()
                    .px_3()
                    .py_1()
                    .justify_between()
                    .text_xs()
                    .text_color(muted)
                    .child(strings.solves)
                    .child(count.to_string()),
            )
            .child(if count == 0 {
                div()
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
                        uniform_list("solves", count, cx.processor(Self::render_solve_rows))
                            .size_full()
                            .track_scroll(&self.solves_scroll),
                    )
                    .child(Scrollbar::vertical(&self.solves_scroll))
                    .into_any_element()
            })
    }

    fn render_session_picker(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings();
        let entity = cx.entity().downgrade();
        let labels: Vec<String> = (0..self.sessions.len())
            .map(|index| self.session_label(index))
            .collect();
        let current = self.current;
        let current_label = labels[current].clone();

        h_flex()
            .p_2()
            .gap_1()
            .child(
                Button::new("session")
                    .outline()
                    .small()
                    .flex_1()
                    .label(current_label)
                    .tab_stop(false)
                    .dropdown_menu(move |mut menu, _, _| {
                        for (index, label) in labels.iter().enumerate() {
                            let entity = entity.clone();
                            menu = menu.item(
                                PopupMenuItem::new(label.clone())
                                    .checked(index == current)
                                    .on_click(move |_, _, cx| {
                                        entity
                                            .update(cx, |app, cx| app.select_session(index, cx))
                                            .ok();
                                    }),
                            );
                        }
                        menu
                    }),
            )
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
                    .icon(IconName::Inbox)
                    .tooltip(strings.rename_session)
                    .tab_stop(false)
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.open_rename_session(window, cx);
                    })),
            )
            .child(
                Button::new("session-delete")
                    .ghost()
                    .small()
                    .icon(IconName::Delete)
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
        let (muted, hover) = (theme.muted_foreground, theme.muted);
        let mono = theme.mono_font_family.clone();
        let solves = self.session().solves();
        let total = solves.len();

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
                        .gap_2()
                        .text_sm()
                        .font_family(mono.clone())
                        .cursor_pointer()
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
                        .child(
                            div()
                                .w(px(64.))
                                .text_right()
                                .text_xs()
                                .text_color(muted)
                                .child(format_average_at(solves, index, 5)),
                        )
                        .on_click(cx.listener(move |app, _, window, cx| {
                            app.open_solve_details(index, window, cx);
                        }))
                        .into_any_element(),
                )
            })
            .collect()
    }

    fn open_rename_session(&self, window: &mut Window, cx: &mut Context<Self>) {
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
        let first_build = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first_build.replace(false) {
                input.focus_handle(cx).focus(window, cx);
            }
            let (input_for_save, entity) = (input.clone(), entity.clone());
            dialog
                .title(strings.rename_session)
                .child(Input::new(&input))
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

    fn confirm_delete_session(&self, window: &mut Window, cx: &mut Context<Self>) {
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
        window.open_dialog(cx, move |dialog, _, _| {
            let on_confirm = std::rc::Rc::clone(&on_confirm);
            dialog.title(title).child(message.clone()).footer(
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
