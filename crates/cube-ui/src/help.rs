//! The keyboard shortcut panel, which is also where bindings are edited.

use cube_core::{Action, BindingError, KeyBinding, Keybinds};
use cube_locale::Strings;
use gpui_kit::assets::IconName;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CubeApp, key_press};
use crate::dialog_size::{DialogSize, dialog_body_height, dialog_width};
use crate::palette::Palette;

/// The panel has action labels, key pills and icon buttons side by side, so it
/// uses the roomier form size rather than the narrower settings size.
const DIALOG_SIZE: DialogSize = DialogSize::FORM;

const TIMER_ACTIONS: [Action; 4] = [
    Action::Timer,
    Action::Cancel,
    Action::NextScramble,
    Action::ToggleInspection,
];
const SOLVE_ACTIONS: [Action; 6] = [
    Action::PreviousSolve,
    Action::NextSolve,
    Action::OpenSolve,
    Action::DeleteSolve,
    Action::PenaltyPlusTwo,
    Action::PenaltyDnf,
];
const SESSION_ACTIONS: [Action; 7] = [
    Action::NextEvent,
    Action::PreviousEvent,
    Action::NextSession,
    Action::PreviousSession,
    Action::NewSession,
    Action::RenameSession,
    Action::DeleteSession,
];
const SCREEN_ACTIONS: [Action; 3] = [Action::OpenStats, Action::OpenSettings, Action::ShowHelp];

/// The shortcut reference, with an edit mode for rebinding.
struct KeybindsView {
    app: Entity<CubeApp>,
    focus: FocusHandle,
    editing: bool,
    /// The action waiting for the user to press its new key.
    capturing: Option<Action>,
    /// Why the last captured key was rejected.
    error: Option<String>,
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
        let theme = cx.theme();
        let border = theme.border;
        let muted = theme.muted_foreground;
        let mono = theme.mono_font_family.clone();
        let colors = Palette::of(cx);

        v_flex()
            .gap_2()
            .child(
                div()
                    .text_xs()
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
                            .min_h(px(36.))
                            .when(index > 0, |row| row.border_t_1().border_color(border))
                            .px_3()
                            .py_2()
                            .gap_3()
                            .justify_between()
                            .text_sm()
                            .child(
                                div()
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
                                            .min_w(px(140.))
                                            .text_center()
                                            .font_family(mono.clone())
                                            .text_xs()
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
            (strings.help_section_screens, &SCREEN_ACTIONS[..]),
        ];
        let capture_hint = self.capturing.map(|action| {
            strings
                .keybind_capture_hint
                .replace("{action}", action_label(strings, action))
        });
        let sections: Vec<AnyElement> = sections
            .into_iter()
            .map(|(heading, actions)| {
                self.render_section(heading, actions, &keybinds, strings, cx)
                    .into_any_element()
            })
            .collect();

        v_flex()
            .id("keybind-panel")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
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
            .when(self.editing, |column| {
                column.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(strings.keybind_hint),
                )
            })
            .when_some(capture_hint, |column, hint| {
                column.child(div().text_xs().text_color(colors.yellow).child(hint))
            })
            .when_some(self.error.clone(), |column, error| {
                column.child(div().text_xs().text_color(colors.red).child(error))
            })
            .children(sections)
            .child(
                div()
                    .px_2()
                    .pt_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(strings.help_hint),
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
        Action::OpenStats => strings.action_open_stats,
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
            KeybindsView {
                app: app.clone(),
                focus: cx.focus_handle(),
                editing: false,
                capturing: None,
                error: None,
            }
        });

        window.open_dialog(cx, move |dialog, window, cx| {
            let editing = view.read(cx).editing;
            let edit_view = view.clone();
            let content_view = view.clone();
            let title = h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .pb_3()
                .child(strings.help_title)
                .child(
                    Button::new("help-close-icon")
                        .ghost()
                        .large()
                        .icon(IconName::Close)
                        .tooltip(strings.close)
                        .accessibility_label(strings.close)
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                );
            dialog
                .title(title)
                .close_button(false)
                .w(dialog_width(DIALOG_SIZE, window))
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
