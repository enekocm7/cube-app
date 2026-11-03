//! The keyboard shortcut reference.

use cube_core::Action;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::Button;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::CubeApp;

/// The tallest the scrolling body gets, however large the window is.
const BODY_MAX_HEIGHT: Pixels = px(480.);
/// The height the dialog needs around the body: its title, its footer and the
/// margins it keeps from the window edges.
const DIALOG_CHROME_HEIGHT: Pixels = px(170.);

impl CubeApp {
    /// Opens the shortcut reference, built from the effective bindings.
    pub(crate) fn open_help(&self, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.strings();
        let keybinds = self.keybinds();
        let label = |action: Action| keybinds.label(action);
        let sections: Vec<(&'static str, Vec<(&'static str, String)>)> = vec![
            (
                strings.help_section_timer,
                vec![
                    (strings.action_timer, label(Action::Timer)),
                    (strings.action_cancel, label(Action::Cancel)),
                    (strings.new_scramble, label(Action::NextScramble)),
                    (
                        strings.action_toggle_inspection,
                        label(Action::ToggleInspection),
                    ),
                ],
            ),
            (
                strings.help_section_solves,
                vec![
                    (strings.action_previous_solve, label(Action::PreviousSolve)),
                    (strings.action_next_solve, label(Action::NextSolve)),
                    (strings.action_open_solve, label(Action::OpenSolve)),
                    (strings.delete_solve, label(Action::DeleteSolve)),
                    (
                        strings.action_penalty_plus_two,
                        label(Action::PenaltyPlusTwo),
                    ),
                    (strings.action_penalty_dnf, label(Action::PenaltyDnf)),
                ],
            ),
            (
                strings.help_section_events_sessions,
                vec![
                    (strings.action_next_event, label(Action::NextEvent)),
                    (strings.action_previous_event, label(Action::PreviousEvent)),
                    (strings.action_next_session, label(Action::NextSession)),
                    (
                        strings.action_previous_session,
                        label(Action::PreviousSession),
                    ),
                    (strings.new_session, label(Action::NewSession)),
                    (strings.rename_session, label(Action::RenameSession)),
                    (strings.delete_session, label(Action::DeleteSession)),
                ],
            ),
            (
                strings.help_section_screens,
                vec![
                    (strings.action_open_stats, label(Action::OpenStats)),
                    (strings.settings, label(Action::OpenSettings)),
                    (strings.action_show_help, label(Action::ShowHelp)),
                ],
            ),
        ];

        window.open_dialog(cx, move |dialog, window, cx| {
            let max_height =
                (window.viewport_size().height - DIALOG_CHROME_HEIGHT).min(BODY_MAX_HEIGHT);
            dialog
                .title(strings.help_title)
                .w(px(520.))
                .child(
                    v_flex()
                        .id("help-body")
                        .max_h(max_height)
                        .p_4()
                        .gap_5()
                        .overflow_y_scrollbar()
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(strings.help_any_key_stops),
                        )
                        .children(
                            sections
                                .clone()
                                .into_iter()
                                .map(|(heading, rows)| section(heading, rows, cx)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(strings.help_hint),
                        ),
                )
                .footer(
                    h_flex().w_full().justify_end().child(
                        Button::new("help-close")
                            .outline()
                            .label(strings.close)
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
                )
        });
    }
}

/// A heading above a card of shortcut rows.
fn section(heading: &'static str, rows: Vec<(&'static str, String)>, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let border = theme.border;
    v_flex()
        .gap_2()
        .child(
            div()
                .text_xs()
                .font_semibold()
                .text_color(theme.muted_foreground)
                .child(heading),
        )
        .child(
            v_flex()
                .rounded(theme.radius_lg)
                .border_1()
                .border_color(border)
                .bg(theme.muted)
                .children(rows.into_iter().enumerate().map(|(index, (label, keys))| {
                    h_flex()
                        .min_h(px(36.))
                        .when(index > 0, |row| row.border_t_1().border_color(border))
                        .px_3()
                        .py_2()
                        .gap_4()
                        .justify_between()
                        .text_sm()
                        .child(div().flex_1().min_w_0().child(label))
                        .child(
                            div()
                                .flex_none()
                                .rounded(theme.radius_lg)
                                .border_1()
                                .border_color(border)
                                .bg(theme.background)
                                .px_2()
                                .py_1()
                                .font_family(theme.mono_font_family.clone())
                                .text_xs()
                                .child(keys),
                        )
                })),
        )
}
