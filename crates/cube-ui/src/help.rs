//! The keyboard shortcut panel, which is also where bindings are edited.

use cube_core::{Action, BindingError, KeyBinding, Keybinds};
use cube_locale::Strings;
use gpui_kit::assets::IconName;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::InputState;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CubeApp, key_press};
use crate::dialog_size::{DialogSize, dialog_body, dialog_body_height, dialog_title, dialog_width};
use crate::dialog_submit::{close_action, route_enter};
use crate::palette::Palette;
use crate::scroll_fade::FadeScroll;
use crate::search_bar::{BAR_HEIGHT, focus_on_slash, new_search, query, search_bar};

/// The panel has action labels, key pills and icon buttons side by side, so it
/// uses the roomier form size rather than the narrower settings size.
const DIALOG_SIZE: DialogSize = DialogSize::FORM;

const TIMER_ACTIONS: [Action; 6] = [
    Action::Timer,
    Action::Cancel,
    Action::NextScramble,
    Action::ToggleInspection,
    Action::ToggleScramblePreview,
    Action::ManualEntry,
];
const SOLVE_ACTIONS: [Action; 11] = [
    Action::PreviousSolve,
    Action::NextSolve,
    Action::OpenSolve,
    Action::DeleteSolve,
    Action::PenaltyPlusTwo,
    Action::PenaltyDnf,
    Action::EditTags,
    Action::ActiveTags,
    Action::ManageTags,
    Action::ShareSolve,
    Action::CopySolveLink,
];
const SESSION_ACTIONS: [Action; 9] = [
    Action::NextEvent,
    Action::PreviousEvent,
    Action::NextSession,
    Action::PreviousSession,
    Action::NewSession,
    Action::RenameSession,
    Action::DeleteSession,
    Action::UndoDelete,
    Action::RedoDelete,
];
const AVERAGE_ACTIONS: [Action; 6] = [
    Action::PreviousAverage,
    Action::NextAverage,
    Action::OpenAverage,
    Action::OpenBestAverage,
    Action::CopyAverage,
    Action::ShareAverage,
];
const SCREEN_ACTIONS: [Action; 8] = [
    Action::OpenStats,
    Action::OpenCharts,
    Action::OpenBluetooth,
    Action::OpenGoals,
    Action::NewGoal,
    Action::OpenTrainer,
    Action::OpenSettings,
    Action::ShowHelp,
];
const TRAINER_ACTIONS: [Action; 9] = [
    Action::NextTrainerSet,
    Action::ToggleTrainerCase,
    Action::ToggleTrainerGroup,
    Action::ToggleTrainerSet,
    Action::StartTrainer,
    Action::RevealTrainerCase,
    Action::EditTrainerAlgorithm,
    Action::SortTrainerCases,
    Action::TrainerCaseAttempts,
];
const CHART_ACTIONS: [Action; 10] = [
    Action::NextChartView,
    Action::NextChartEvent,
    Action::NextChartRange,
    Action::NextTagFilter,
    Action::NextCompareTag,
    Action::ChartDates,
    Action::ToggleChartSessions,
    Action::ChartZoomIn,
    Action::ChartZoomOut,
    Action::ChartResetZoom,
];

/// The shortcut reference, with an edit mode for rebinding.
struct KeybindsView {
    app: Entity<CubeApp>,
    focus: FocusHandle,
    editing: bool,
    /// The action waiting for the user to press its new key.
    capturing: Option<Action>,
    /// Why the last captured key was rejected.
    error: Option<String>,
    /// Filters the rows by label and binding.
    search: Entity<InputState>,
}

impl KeybindsView {
    /// Switches between reading and editing the shortcuts.
    fn toggle_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing = !self.editing;
        if !self.editing {
            self.capturing = None;
            self.error = None;
        }
        self.focus.focus(window, cx);
        // The footer's Edit/Done label lives outside this view, so refresh the
        // whole window to rebuild it.
        window.refresh();
        cx.notify();
    }

    /// Shows a rejected key change inline and as a notification.
    fn report_binding_error(
        &mut self,
        message: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.push_notification(Notification::error(message.clone()), cx);
        self.error = Some(message);
        cx.notify();
    }

    /// Starts waiting for the key that will replace `action`'s binding.
    fn start_capture(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        self.capturing = Some(action);
        self.error = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Removes an action's binding and saves the change.
    fn remove_binding(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.app.read(cx).strings();
        match self
            .app
            .update(cx, |app, cx| app.set_keybinding(action, None, window, cx))
        {
            Ok(()) => self.error = None,
            Err(error) => {
                self.report_binding_error(binding_error_message(strings, error), window, cx);
            }
        }
        self.capturing = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn cancel_capture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.capturing = None;
        self.error = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Captures the next key while an action is being rebound.
    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(action) = self.capturing else {
            return;
        };
        cx.stop_propagation();
        if event.is_held {
            return;
        }
        // Escape backs out; the dialog itself would otherwise close.
        if event.keystroke.key == "escape" {
            self.cancel_capture(window, cx);
            return;
        }
        let Some(press) = key_press(&event.keystroke) else {
            let message = self.app.read(cx).strings().keybind_error_invalid.to_owned();
            self.report_binding_error(message, window, cx);
            return;
        };
        let binding = KeyBinding::from_press(&press);
        let strings = self.app.read(cx).strings();
        match self.app.update(cx, |app, cx| {
            app.set_keybinding(action, Some(binding), window, cx)
        }) {
            Ok(()) => {
                self.capturing = None;
                self.error = None;
                self.focus.focus(window, cx);
                cx.notify();
            }
            Err(error) => {
                self.report_binding_error(binding_error_message(strings, error), window, cx);
            }
        }
    }

    /// One action per row: its keys, and in edit mode the pencil and trash.
    #[allow(
        clippy::too_many_lines,
        reason = "one row per action with its controls"
    )]
    fn render_section(
        &self,
        heading: &'static str,
        actions: &[Action],
        keybinds: &Keybinds,
        strings: &'static Strings,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Only the actions the search keeps; a section left with none is
        // skipped by the caller.
        let theme = cx.theme();
        let border = theme.border;
        let muted = theme.muted_foreground;
        let mono = theme.mono_font_family.clone();
        let colors = Palette::of(cx);

        v_flex()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .text_color(muted)
                    .child(heading),
            )
            .child(
                v_flex()
                    .rounded(theme.radius_lg)
                    .border_1()
                    .border_color(border)
                    .bg(theme.muted)
                    .children(actions.iter().enumerate().map(|(index, &action)| {
                        let capturing = self.capturing == Some(action);
                        let binding = keybinds.label(action);
                        let bound = binding.is_some();
                        let can_remove = action != Action::Timer && bound;
                        let keys = if capturing {
                            strings.keybind_capture.to_owned()
                        } else {
                            binding.unwrap_or_else(|| strings.unassigned.to_owned())
                        };
                        let keys_color = if capturing {
                            colors.yellow
                        } else if bound {
                            theme.foreground
                        } else {
                            muted
                        };
                        h_flex()
                            .min_h(Rems(2.25))
                            .when(index > 0, |row| row.border_t_1().border_color(border))
                            .px_3()
                            .py_2()
                            .gap_3()
                            .justify_between()
                            .text_base()
                            .child(
                                div()
                                    .text_base()
                                    .flex_1()
                                    .min_w_0()
                                    .child(action_label(strings, action)),
                            )
                            .child(
                                h_flex()
                                    .flex_none()
                                    .gap_2()
                                    .child(
                                        div()
                                            .min_w(Rems(8.75))
                                            .text_center()
                                            .font_family(mono.clone())
                                            .text_base()
                                            .text_color(keys_color)
                                            .child(keys),
                                    )
                                    .when(self.editing && !capturing, |row| {
                                        row.child(
                                            Button::new(format!(
                                                "keybind-change-{}",
                                                action.name()
                                            ))
                                            .ghost()
                                            .small()
                                            .icon(IconName::Pencil)
                                            .tooltip(strings.keybind_change)
                                            .accessibility_label(strings.keybind_change)
                                            .on_click(cx.listener(move |view, _, window, cx| {
                                                view.start_capture(action, window, cx);
                                            })),
                                        )
                                    })
                                    .when(self.editing && !capturing && can_remove, |row| {
                                        row.child(
                                            Button::new(format!(
                                                "keybind-remove-{}",
                                                action.name()
                                            ))
                                            .danger()
                                            .small()
                                            .icon(IconName::Trash)
                                            .tooltip(strings.keybind_remove)
                                            .accessibility_label(strings.keybind_remove)
                                            .on_click(cx.listener(move |view, _, window, cx| {
                                                view.remove_binding(action, window, cx);
                                            })),
                                        )
                                    })
                                    .when(capturing, |row| {
                                        row.child(
                                            Button::new(format!(
                                                "keybind-cancel-{}",
                                                action.name()
                                            ))
                                            .ghost()
                                            .icon(IconName::X)
                                            .tooltip(strings.cancel)
                                            .accessibility_label(strings.cancel)
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.cancel_capture(window, cx);
                                            })),
                                        )
                                    }),
                            )
                            .into_any_element()
                    })),
            )
    }
}

impl Render for KeybindsView {
    #[allow(clippy::too_many_lines, reason = "one section per group of actions")]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.app.read(cx);
        let strings = app.strings();
        let keybinds = app.keybinds().clone();
        let colors = Palette::of(cx);
        let max_height = dialog_body_height(DIALOG_SIZE, window);
        let sections = [
            (strings.help_section_timer, &TIMER_ACTIONS[..]),
            (strings.help_section_solves, &SOLVE_ACTIONS[..]),
            (strings.help_section_events_sessions, &SESSION_ACTIONS[..]),
            (strings.help_section_averages, &AVERAGE_ACTIONS[..]),
            (strings.help_section_screens, &SCREEN_ACTIONS[..]),
            (strings.help_section_charts, &CHART_ACTIONS[..]),
            (strings.nav_trainer, &TRAINER_ACTIONS[..]),
        ];
        let capture_hint = self.capturing.map(|action| {
            strings
                .keybind_capture_hint
                .replace("{action}", action_label(strings, action))
        });
        let search = query(&self.search, cx);
        let sections: Vec<AnyElement> = sections
            .into_iter()
            .filter_map(|(heading, actions)| {
                let actions: Vec<Action> = actions
                    .iter()
                    .copied()
                    .filter(|&action| {
                        // The row being rebound stays, so the capture is never lost.
                        self.capturing == Some(action)
                            || cube_core::search::matches(
                                &search,
                                &[
                                    action_label(strings, action).to_owned(),
                                    keybinds
                                        .label(action)
                                        .unwrap_or_else(|| strings.unassigned.to_owned()),
                                ],
                            )
                    })
                    .collect();
                (!actions.is_empty()).then(|| {
                    self.render_section(heading, &actions, &keybinds, strings, cx)
                        .into_any_element()
                })
            })
            .collect();
        let no_results = sections.is_empty();

        let bar = search_bar(&self.search, cx);
        dialog_body(
            v_flex()
                .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                    if focus_on_slash(event, &view.search, window, cx) {
                        cx.stop_propagation();
                    }
                }))
                .child(bar)
                .child(
                    FadeScroll::new("keybind-panel", cx.theme().background)
                        .track_focus(&self.focus)
                        .on_key_down(cx.listener(Self::on_key_down))
                        .max_h(max_height - BAR_HEIGHT.to_pixels(window.rem_size()))
                        .p_4()
                        .gap_5()
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(strings.help_any_key_stops),
                        )
                        .when(self.editing, |column| {
                            column.child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(strings.keybind_hint),
                            )
                        })
                        .when_some(capture_hint, |column, hint| {
                            column.child(div().text_sm().text_color(colors.yellow).child(hint))
                        })
                        .when_some(self.error.clone(), |column, error| {
                            column.child(div().text_sm().text_color(colors.red).child(error))
                        })
                        .children(sections)
                        .when(no_results, |column| {
                            column.child(
                                div()
                                    .text_base()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(strings.search_no_results),
                            )
                        })
                        .child(
                            div()
                                .px_2()
                                .pt_2()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(strings.help_hint),
                        ),
                ),
        )
    }
}

/// Returns the translated description of an action.
fn action_label(strings: &'static Strings, action: Action) -> &'static str {
    match action {
        Action::Timer => strings.action_timer,
        Action::Cancel => strings.action_cancel,
        Action::NextScramble => strings.new_scramble,
        Action::ToggleInspection => strings.action_toggle_inspection,
        Action::ToggleScramblePreview => strings.action_toggle_scramble_preview,
        Action::ManualEntry => strings.action_manual_entry,
        Action::PreviousSolve => strings.action_previous_solve,
        Action::NextSolve => strings.action_next_solve,
        Action::OpenSolve => strings.action_open_solve,
        Action::DeleteSolve => strings.delete_solve,
        Action::PenaltyPlusTwo => strings.action_penalty_plus_two,
        Action::PenaltyDnf => strings.action_penalty_dnf,
        Action::NextEvent => strings.action_next_event,
        Action::PreviousEvent => strings.action_previous_event,
        Action::NextSession => strings.action_next_session,
        Action::PreviousSession => strings.action_previous_session,
        Action::NewSession => strings.new_session,
        Action::RenameSession => strings.rename_session,
        Action::DeleteSession => strings.delete_session,
        Action::UndoDelete => strings.action_undo_delete,
        Action::RedoDelete => strings.action_redo_delete,
        Action::OpenStats => strings.action_open_stats,
        Action::OpenCharts => strings.action_open_charts,
        Action::OpenBluetooth => strings.action_open_bluetooth,
        Action::OpenGoals => strings.action_open_goals,
        Action::NewGoal => strings.goals_new,
        Action::OpenTrainer => strings.action_open_trainer,
        Action::NextTrainerSet => strings.action_next_trainer_set,
        Action::ToggleTrainerCase => strings.trainer_toggle_case,
        Action::ToggleTrainerGroup => strings.trainer_toggle_group,
        Action::ToggleTrainerSet => strings.trainer_toggle_all,
        Action::StartTrainer => strings.trainer_start,
        Action::RevealTrainerCase => strings.trainer_reveal,
        Action::EditTrainerAlgorithm => strings.trainer_edit_algorithm,
        Action::SortTrainerCases => strings.trainer_sort_slowest,
        Action::TrainerCaseAttempts => strings.trainer_case_attempts,
        Action::EditTags => strings.action_edit_tags,
        Action::ActiveTags => strings.action_active_tags,
        Action::ManageTags => strings.tags_manage,
        Action::NextTagFilter => strings.action_next_tag_filter,
        Action::NextCompareTag => strings.action_next_compare_tag,
        Action::NextChartView => strings.action_next_chart_view,
        Action::NextChartEvent => strings.action_next_chart_event,
        Action::NextChartRange => strings.action_next_chart_range,
        Action::ChartDates => strings.action_chart_dates,
        Action::ToggleChartSessions => strings.action_toggle_chart_sessions,
        Action::ChartZoomIn => strings.action_chart_zoom_in,
        Action::ChartZoomOut => strings.action_chart_zoom_out,
        Action::ChartResetZoom => strings.action_chart_reset_zoom,
        Action::PreviousAverage => strings.action_previous_average,
        Action::NextAverage => strings.action_next_average,
        Action::OpenAverage => strings.action_open_average,
        Action::OpenBestAverage => strings.action_open_best_average,
        Action::CopyAverage => strings.action_copy_average,
        Action::ShareSolve => strings.action_share_solve,
        Action::ShareAverage => strings.action_share_average,
        Action::CopySolveLink => strings.action_copy_solve_link,
        Action::OpenSettings => strings.settings,
        Action::ShowHelp => strings.action_show_help,
    }
}

/// Turns a rejected key change into the message shown in the panel.
fn binding_error_message(strings: &'static Strings, error: BindingError) -> String {
    match error {
        BindingError::TimerModified => strings.keybind_error_timer_modified.to_owned(),
        BindingError::Conflict(action) => strings
            .keybind_error_conflict
            .replace("{action}", action_label(strings, action)),
        BindingError::TimerRequired => strings.keybind_error_timer_required.to_owned(),
    }
}

impl CubeApp {
    /// Opens the shortcut panel, where the bindings can also be edited.
    pub(crate) fn open_help(&self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity();
        let strings = self.strings();
        let view = cx.new(|cx| {
            cx.observe(&app, |_, _, cx| cx.notify()).detach();
            let search = new_search(strings, window, cx);
            KeybindsView {
                app: app.clone(),
                focus: cx.focus_handle(),
                editing: false,
                capturing: None,
                error: None,
                search,
            }
        });

        let first_build = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first_build.replace(false) {
                // So `/` and capture keys reach the panel before any click.
                let focus = view.read(cx).focus.clone();
                focus.focus(window, cx);
            }
            let editing = view.read(cx).editing;
            let edit_view = view.clone();
            let content_view = view.clone();
            let (enter_view, escape_view) = (view.clone(), view.clone());
            let close = close_action();
            let title = dialog_title(strings.help_title, strings.close);
            dialog
                .title(title)
                .close_button(false)
                .w(dialog_width(DIALOG_SIZE, window))
                // Key bindings fire first, so a key being captured has to be
                // passed on or it would reach the dialog instead of the view.
                .on_ok(move |_, window, cx| {
                    if enter_view.read(cx).capturing.is_some() {
                        cx.propagate();
                        return false;
                    }
                    route_enter(&close, window, cx)
                })
                .on_cancel(move |_, _, cx| {
                    let capturing = escape_view.read(cx).capturing.is_some();
                    if capturing {
                        cx.propagate();
                    }
                    !capturing
                })
                .content(move |content, _, cx| {
                    content
                        .px_0()
                        .rounded_none()
                        .border_y_1()
                        .border_color(cx.theme().border)
                        .child(content_view.clone())
                })
                .footer(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .child(
                            Button::new("help-close")
                                .outline()
                                .label(strings.close)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("keybind-edit")
                                .outline()
                                .label(if editing {
                                    strings.keybind_done
                                } else {
                                    strings.keybind_edit
                                })
                                .on_click(move |_, window, cx| {
                                    edit_view.update(cx, |view, cx| {
                                        view.toggle_editing(window, cx);
                                    });
                                }),
                        ),
                )
        });
    }
}
