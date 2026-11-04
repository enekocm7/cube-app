//! The settings dialog.

use cube_core::persistence;
use cube_core::settings::{ThemePreference, UpdateChannel, UpdateFrequency};
use cube_core::theme;
use cube_core::theme::{ThemeColor, ThemeFile};
use cube_core::{Action, BindingError, KeyBinding, Keybinds, Settings};
use cube_locale::{Language, Strings};
use gpui_kit::assets::IconName;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{
    ActiveTheme as _, Selectable as _, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CubeApp, key_press};
use crate::help::action_label;
use crate::palette::Palette;

/// The tallest the scrolling body gets, however large the window is.
const BODY_MAX_HEIGHT: Pixels = px(460.);
/// The height the dialog needs around the body: its title, its footer and the
/// margins it keeps from the window edges.
const DIALOG_CHROME_HEIGHT: Pixels = px(170.);
const ROW_MIN_HEIGHT: Pixels = px(44.);

/// A view of the app's settings; re-renders whenever the app changes.
struct SettingsView {
    app: Entity<CubeApp>,
    /// Every valid theme in the shared `themes` folder, read when the dialog
    /// opens.
    themes: Vec<ThemeFile>,
    focus: FocusHandle,
    /// The action waiting for the user to press its new key.
    capturing: Option<Action>,
    /// Why the last captured key was rejected.
    error: Option<String>,
}

impl SettingsView {
    /// Opens a picker and imports the chosen csTimer export.
    fn import(&self, window: &mut Window, cx: &mut Context<Self>) {
        let prompt = self.app.read(cx).strings().import_prompt;
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(prompt.into()),
        });
        let app = self.app.clone();
        cx.spawn_in(window, async move |_, cx| match receiver.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.first() {
                    let _ = app.update_in(cx, |app, window, cx| {
                        app.import_cstimer(path, window, cx);
                    });
                }
            }
            Ok(Err(error)) => {
                let _ = app.update_in(cx, |app, window, cx| {
                    app.report_import_error(&error, window, cx);
                });
            }
            // Cancelled, or the window closed while the picker was open.
            Ok(Ok(None)) | Err(_) => {}
        })
        .detach();
    }

    /// Opens a save picker and exports every session as csTimer JSON.
    fn export(&self, window: &mut Window, cx: &mut Context<Self>) {
        let directory = dirs::document_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_default();
        let receiver = cx.prompt_for_new_path(&directory, Some("cstimer.json"));
        let app = self.app.clone();
        cx.spawn_in(window, async move |_, cx| match receiver.await {
            Ok(Ok(Some(path))) => {
                let _ = app.update_in(cx, |app, window, cx| {
                    app.export_cstimer(&path, window, cx);
                });
            }
            Ok(Err(error)) => {
                let _ = app.update_in(cx, |app, window, cx| {
                    app.report_export_error(&error, window, cx);
                });
            }
            // Cancelled, or the window closed while the picker was open.
            Ok(Ok(None)) | Err(_) => {}
        })
        .detach();
    }

    fn change(
        &self,
        window: &mut Window,
        cx: &mut App,
        change: impl FnOnce(&mut Settings) + 'static,
    ) {
        self.app.update(cx, |app, cx| {
            app.update_settings(window, cx, change);
        });
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
            Err(error) => self.error = Some(binding_error_message(strings, error)),
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
            self.error = Some(self.app.read(cx).strings().keybind_error_invalid.to_owned());
            cx.notify();
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
            }
            Err(error) => self.error = Some(binding_error_message(strings, error)),
        }
        cx.notify();
    }

    /// One row per action: its current key, a Change button and, when allowed,
    /// a Remove button.
    fn keybind_rows(
        &self,
        keybinds: &Keybinds,
        strings: &'static Strings,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let colors = Palette::of(cx);
        let theme = cx.theme();
        let mono = theme.mono_font_family.clone();
        Action::ALL
            .into_iter()
            .map(|action| {
                let capturing = self.capturing == Some(action);
                let binding = keybinds.label(action);
                let bound = binding.is_some();
                let can_remove = action != Action::Timer && bound;
                let text = if capturing {
                    strings.keybind_capture.to_owned()
                } else {
                    binding.unwrap_or_else(|| strings.unassigned.to_owned())
                };
                let text_color = if capturing {
                    colors.yellow
                } else if bound {
                    theme.foreground
                } else {
                    theme.muted_foreground
                };
                h_flex()
                    .min_h(ROW_MIN_HEIGHT)
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
                                    .min_w(px(110.))
                                    .text_center()
                                    .font_family(mono.clone())
                                    .text_xs()
                                    .text_color(text_color)
                                    .child(text),
                            )
                            .when(!capturing, |row| {
                                row.child(
                                    Button::new(format!("keybind-change-{}", action.name()))
                                        .small()
                                        .outline()
                                        .label(strings.keybind_change)
                                        .on_click(cx.listener(move |view, _, window, cx| {
                                            view.start_capture(action, window, cx);
                                        })),
                                )
                            })
                            .when(can_remove, |row| {
                                row.child(
                                    Button::new(format!("keybind-remove-{}", action.name()))
                                        .small()
                                        .ghost()
                                        .label(strings.keybind_remove)
                                        .on_click(cx.listener(move |view, _, window, cx| {
                                            view.remove_binding(action, window, cx);
                                        })),
                                )
                            })
                            .when(capturing, |row| {
                                row.child(
                                    Button::new(format!("keybind-cancel-{}", action.name()))
                                        .small()
                                        .outline()
                                        .label(strings.cancel)
                                        .on_click(cx.listener(|view, _, window, cx| {
                                            view.cancel_capture(window, cx);
                                        })),
                                )
                            }),
                    )
                    .into_any_element()
            })
            .collect()
    }

    /// The csTimer import and export actions.
    fn data_rows(&self, strings: &'static Strings, cx: &mut Context<Self>) -> [AnyElement; 2] {
        [
            row(
                strings.setting_import,
                Button::new("import-cstimer")
                    .small()
                    .outline()
                    .label(strings.import_button)
                    .on_click(cx.listener(|view, _, window, cx| view.import(window, cx))),
            ),
            row(
                strings.setting_export,
                Button::new("export-cstimer")
                    .small()
                    .outline()
                    .label(strings.export_button)
                    .on_click(cx.listener(|view, _, window, cx| view.export(window, cx))),
            ),
        ]
    }

    fn switch(
        &self,
        id: &'static str,
        label: &'static str,
        checked: bool,
        set: fn(&mut Settings, bool),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        row(
            label,
            Switch::new(id).checked(checked).on_click(cx.listener(
                move |view, value: &bool, window, cx| {
                    let value = *value;
                    view.change(window, cx, move |settings| set(settings, value));
                },
            )),
        )
    }

    fn languages(&self, current: &str, cx: &mut Context<Self>) -> Vec<Button> {
        Language::ALL
            .into_iter()
            .map(|language| {
                choice(
                    language.code(),
                    language.native_name(),
                    Language::from_code(current) == Some(language),
                )
                .on_click(cx.listener(move |view, _, window, cx| {
                    view.change(window, cx, move |settings| {
                        language.code().clone_into(&mut settings.language);
                    });
                }))
            })
            .collect()
    }

    /// The updater's preferences and its manual check.
    #[allow(clippy::too_many_lines, reason = "one row per updater preference")]
    fn update_rows(
        &self,
        strings: &'static Strings,
        settings: &Settings,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let channel_button = |id: &'static str,
                              label: &'static str,
                              channel: UpdateChannel,
                              cx: &mut Context<Self>| {
            choice(id, label, settings.updates.channel == channel).on_click(cx.listener(
                move |view, _, window, cx| {
                    view.change(window, cx, move |settings| {
                        settings.updates.channel = channel;
                    });
                },
            ))
        };
        let frequency_button = |id: &'static str,
                                label: &'static str,
                                frequency: UpdateFrequency,
                                cx: &mut Context<Self>| {
            choice(id, label, settings.updates.frequency == frequency).on_click(cx.listener(
                move |view, _, window, cx| {
                    view.change(window, cx, move |settings| {
                        settings.updates.frequency = frequency;
                    });
                },
            ))
        };
        let mut rows = vec![
            self.switch(
                "auto-updates",
                strings.setting_check_updates,
                settings.updates.auto_check,
                |settings, value| settings.updates.auto_check = value,
                cx,
            ),
            row(
                strings.setting_update_channel,
                h_flex()
                    .gap_2()
                    .child(channel_button(
                        "update-stable",
                        strings.update_channel_stable,
                        UpdateChannel::Stable,
                        cx,
                    ))
                    .child(channel_button(
                        "update-prerelease",
                        strings.update_channel_prerelease,
                        UpdateChannel::Prerelease,
                        cx,
                    )),
            ),
            row(
                strings.setting_update_frequency,
                h_flex()
                    .gap_2()
                    .child(frequency_button(
                        "update-freq-start",
                        strings.update_frequency_start,
                        UpdateFrequency::Start,
                        cx,
                    ))
                    .child(frequency_button(
                        "update-freq-daily",
                        strings.update_frequency_daily,
                        UpdateFrequency::Daily,
                        cx,
                    ))
                    .child(frequency_button(
                        "update-freq-weekly",
                        strings.update_frequency_weekly,
                        UpdateFrequency::Weekly,
                        cx,
                    )),
            ),
            row(
                strings.setting_check_now,
                Button::new("check-updates")
                    .small()
                    .outline()
                    .label(strings.setting_check_now)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.app
                            .update(cx, |app, cx| app.check_for_updates(true, window, cx));
                    })),
            ),
        ];
        if let Some(version) = settings.updates.skipped_version.clone() {
            rows.push(
                h_flex()
                    .min_h(ROW_MIN_HEIGHT)
                    .px_3()
                    .py_2()
                    .gap_4()
                    .justify_between()
                    .text_sm()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(strings.setting_skipping.replace("{version}", &version)),
                    )
                    .child(
                        Button::new("clear-skip")
                            .small()
                            .outline()
                            .label(strings.setting_clear_skip)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.change(window, cx, |settings| {
                                    settings.updates.skipped_version = None;
                                });
                            })),
                    )
                    .into_any_element(),
            );
        }
        rows
    }

    /// One row per theme file, with controls to apply, edit or delete it.
    fn theme_list_rows(
        &self,
        settings: &Settings,
        strings: &'static cube_locale::Strings,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        if self.themes.is_empty() {
            return vec![
                h_flex()
                    .min_h(ROW_MIN_HEIGHT)
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(strings.no_themes)
                    .into_any_element(),
            ];
        }
        self.themes
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let file = entry.name.clone();
                let display = theme::display_name(&file).to_owned();
                let colors = entry.colors;
                let selected = settings.theme_file.eq_ignore_ascii_case(&file);
                let apply_file = file.clone();
                let edit_file = file.clone();
                let delete_file = file.clone();
                let delete_display = display.clone();
                h_flex()
                    .min_h(ROW_MIN_HEIGHT)
                    .px_3()
                    .py_2()
                    .gap_2()
                    .text_sm()
                    .child(
                        h_flex()
                            .w(px(36.))
                            .h(px(18.))
                            .flex_shrink_0()
                            .overflow_hidden()
                            .rounded(px(3.))
                            .border_1()
                            .border_color(theme_hsla(colors.border))
                            .child(div().flex_1().h_full().bg(theme_hsla(colors.background)))
                            .child(div().flex_1().h_full().bg(theme_hsla(colors.selection)))
                            .child(div().flex_1().h_full().bg(theme_hsla(colors.accent))),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            Button::new(("theme-apply", index))
                                .ghost()
                                .small()
                                .w_full()
                                .label(display)
                                .selected(selected)
                                .tab_stop(false)
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    let file = apply_file.clone();
                                    view.change(window, cx, move |settings| {
                                        file.clone_into(&mut settings.theme_file);
                                    });
                                })),
                        ),
                    )
                    .child(
                        Button::new(("theme-edit", index))
                            .ghost()
                            .small()
                            .icon(IconName::Pencil)
                            .tooltip(strings.edit_theme)
                            .tab_stop(false)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                let file = edit_file.clone();
                                view.app.update(cx, |app, cx| {
                                    app.open_theme_editor(Some(file), window, cx);
                                });
                            })),
                    )
                    .child(
                        Button::new(("theme-delete", index))
                            .danger()
                            .small()
                            .icon(IconName::Trash)
                            .tooltip(strings.delete_theme)
                            .tab_stop(false)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.confirm_delete_theme(
                                    &delete_file,
                                    &delete_display,
                                    window,
                                    cx,
                                );
                            })),
                    )
                    .into_any_element()
            })
            .collect()
    }

    fn new_theme_row(strings: &'static cube_locale::Strings, cx: &mut Context<Self>) -> AnyElement {
        row(
            strings.custom_themes,
            Button::new("theme-new")
                .primary()
                .small()
                .label(strings.new_theme)
                .on_click(cx.listener(|view, _, window, cx| {
                    view.app.update(cx, |app, cx| {
                        app.open_theme_editor(None, window, cx);
                    });
                })),
        )
    }

    fn confirm_delete_theme(
        &self,
        file: &str,
        display: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let strings = self.app.read(cx).strings();
        let message = strings.theme_delete_confirm.replace("{name}", display);
        let app = self.app.clone();
        let file = file.to_owned();
        let display = display.to_owned();
        window.open_dialog(cx, move |dialog, _, _| {
            let app = app.clone();
            let file = file.clone();
            let display = display.clone();
            dialog
                .title(strings.delete_theme)
                .child(message.clone())
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("theme-delete-cancel")
                                .outline()
                                .label(strings.cancel)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("theme-delete-confirm")
                                .danger()
                                .label(strings.delete)
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    let app = app.clone();
                                    let file = file.clone();
                                    let display = display.clone();
                                    let dir = match persistence::themes_dir() {
                                        Ok(dir) => dir,
                                        Err(error) => {
                                            let message = strings
                                                .theme_delete_failed
                                                .replace("{error}", &format!("{error:#}"));
                                            app.update(cx, |app, cx| {
                                                app.toast(&message, window, cx);
                                            });
                                            return;
                                        }
                                    };
                                    match theme::delete(&dir, &file) {
                                        Ok(()) => {
                                            app.update(cx, |app, cx| {
                                                app.theme_deleted(&file, &display, window, cx);
                                            });
                                        }
                                        Err(error) => {
                                            let message = strings
                                                .theme_delete_failed
                                                .replace("{error}", &format!("{error:#}"));
                                            app.update(cx, |app, cx| {
                                                app.toast(&message, window, cx);
                                            });
                                        }
                                    }
                                }),
                        ),
                )
        });
    }

    /// The built-in light/dark choices. A custom theme takes precedence, so
    /// none of them looks selected while one is active.
    fn theme_row(
        &self,
        settings: &Settings,
        strings: &'static cube_locale::Strings,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let button = |id: &'static str,
                      label: &'static str,
                      theme: ThemePreference,
                      cx: &mut Context<Self>| {
            choice(
                id,
                label,
                settings.theme_file.trim().is_empty() && settings.theme == theme,
            )
            .on_click(cx.listener(move |view, _, window, cx| {
                view.change(window, cx, move |settings| {
                    settings.theme = theme;
                    settings.theme_file.clear();
                });
            }))
        };
        row(
            strings.setting_theme,
            h_flex()
                .gap_2()
                .child(button(
                    "theme-system",
                    strings.theme_system,
                    ThemePreference::System,
                    cx,
                ))
                .child(button(
                    "theme-light",
                    strings.theme_light,
                    ThemePreference::Light,
                    cx,
                ))
                .child(button(
                    "theme-dark",
                    strings.theme_dark,
                    ThemePreference::Dark,
                    cx,
                )),
        )
    }
}

impl Render for SettingsView {
    #[allow(clippy::too_many_lines, reason = "one section per settings group")]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Theme files can be added, renamed or deleted while the dialog is
        // open, so read them again on every render.
        self.themes = persistence::list_themes().unwrap_or_default();
        let app = self.app.read(cx);
        let strings = app.strings();
        let settings = app.settings.clone();
        let keybinds = app.keybinds().clone();
        let colors = Palette::of(cx);

        let mut keybind_rows = vec![
            h_flex()
                .px_3()
                .py_2()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(strings.keybind_hint)
                .into_any_element(),
        ];
        if let Some(action) = self.capturing {
            keybind_rows.push(
                h_flex()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(colors.yellow)
                    .child(
                        strings
                            .keybind_capture_hint
                            .replace("{action}", action_label(strings, action)),
                    )
                    .into_any_element(),
            );
        }
        if let Some(error) = self.error.clone() {
            keybind_rows.push(
                h_flex()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(colors.red)
                    .child(error)
                    .into_any_element(),
            );
        }
        keybind_rows.extend(self.keybind_rows(&keybinds, strings, cx));

        let languages = self.languages(&settings.language, cx);
        let mut theme_rows = vec![
            self.theme_row(&settings, strings, cx),
            Self::new_theme_row(strings, cx),
        ];
        theme_rows.extend(self.theme_list_rows(&settings, strings, cx));

        let timer = [
            self.switch(
                "inspection",
                strings.setting_inspection,
                settings.inspection,
                |settings, value| settings.inspection = value,
                cx,
            ),
            self.switch(
                "inspection-audio",
                strings.setting_inspection_audio,
                settings.inspection_audio,
                |settings, value| settings.inspection_audio = value,
                cx,
            ),
            self.switch(
                "accurate",
                strings.setting_accurate,
                settings.accurate_3x3,
                |settings, value| settings.accurate_3x3 = value,
                cx,
            ),
        ];
        let appearance = [
            self.switch(
                "solve-list-ao5",
                strings.setting_solve_list_ao5,
                settings.solve_list_ao5,
                |settings, value| settings.solve_list_ao5 = value,
                cx,
            ),
            self.switch(
                "toasts",
                strings.setting_toasts,
                settings.toasts,
                |settings, value| settings.toasts = value,
                cx,
            ),
        ];
        let updates = self.update_rows(strings, &settings, cx);
        let language = [h_flex()
            .min_h(ROW_MIN_HEIGHT)
            .px_3()
            .gap_2()
            .children(languages)
            .into_any_element()];
        let data = self.data_rows(strings, cx);

        let max_height =
            (window.viewport_size().height - DIALOG_CHROME_HEIGHT).min(BODY_MAX_HEIGHT);

        v_flex()
            .id("settings-body")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .max_h(max_height)
            .p_4()
            .gap_5()
            .child(section(strings.settings_timer, timer, cx))
            .child(section(strings.settings_theme, theme_rows, cx))
            .child(section(strings.settings_appearance, appearance, cx))
            .child(section(strings.settings_keybindings, keybind_rows, cx))
            .child(section(strings.settings_updates, updates, cx))
            .child(section(strings.setting_language, language, cx))
            .child(section(strings.settings_data, data, cx))
            .overflow_y_scrollbar()
    }
}

/// A heading above a card of rows, split by hairlines.
fn section(
    heading: &'static str,
    rows: impl IntoIterator<Item = AnyElement>,
    cx: &App,
) -> impl IntoElement {
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
                .children(rows.into_iter().enumerate().map(|(index, row)| {
                    div()
                        .when(index > 0, |row| row.border_t_1().border_color(border))
                        .child(row)
                })),
        )
}

/// One option of a group, filled while it is the current one.
fn choice(id: impl Into<ElementId>, label: impl Into<SharedString>, selected: bool) -> Button {
    let button = Button::new(id).small().label(label).selected(selected);
    if selected {
        button.primary()
    } else {
        button.outline()
    }
}

/// Turns a rejected key change into the message shown in Settings.
fn binding_error_message(strings: &'static Strings, error: BindingError) -> String {
    match error {
        BindingError::TimerModified => strings.keybind_error_timer_modified.to_owned(),
        BindingError::Conflict(action) => strings
            .keybind_error_conflict
            .replace("{action}", action_label(strings, action)),
        BindingError::TimerRequired => strings.keybind_error_timer_required.to_owned(),
    }
}

/// Converts a theme file color for use in the interface.
fn theme_hsla(color: ThemeColor) -> Hsla {
    rgb(color.rgb()).into()
}

/// A label on the left of its control.
fn row(label: &'static str, control: impl IntoElement) -> AnyElement {
    h_flex()
        .min_h(ROW_MIN_HEIGHT)
        .px_3()
        .py_2()
        .gap_4()
        .justify_between()
        .text_sm()
        .child(div().flex_1().min_w_0().child(label))
        .child(div().flex_none().child(control))
        .into_any_element()
}

impl CubeApp {
    pub(crate) fn open_settings(&self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity();
        // cube-tui creates this file too, so both apps start with a theme to
        // pick from.
        let _ = persistence::ensure_default_theme();
        let themes = persistence::list_themes().unwrap_or_default();
        let view = cx.new(|cx| {
            cx.observe(&app, |_, _, cx| cx.notify()).detach();
            SettingsView {
                app: app.clone(),
                themes,
                focus: cx.focus_handle(),
                capturing: None,
                error: None,
            }
        });
        let strings_app = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let strings = strings_app.read(cx).strings();
            let view = view.clone();
            dialog
                .title(div().text_lg().pb_2().child(strings.settings))
                .w(px(620.))
                .content(move |content, _, cx| {
                    content
                        .px_0()
                        .rounded_none()
                        .border_y_1()
                        .border_color(cx.theme().border)
                        .child(view.clone())
                })
                .footer(
                    h_flex().w_full().justify_end().child(
                        Button::new("settings-close")
                            .outline()
                            .label(strings.close)
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
                )
        });
    }
}
