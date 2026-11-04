//! The settings dialog.

use cube_core::Settings;
use cube_core::persistence;
use cube_core::settings::{FontChoice, ThemePreference, UpdateChannel, UpdateFrequency};
use cube_core::theme;
use cube_core::theme::{ThemeColor, ThemeFile};
use cube_locale::{Language, Strings};
use gpui_kit::assets::IconName;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{
    ActiveTheme as _, Selectable as _, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::CubeApp;
use crate::dialog_size::{DialogSize, dialog_body, dialog_body_height, dialog_title, dialog_width};
use crate::scroll_fade::FadeScroll;

const ROW_MIN_HEIGHT: Pixels = px(44.);

/// A view of the app's settings; re-renders whenever the app changes.
struct SettingsView {
    app: Entity<CubeApp>,
    /// Every valid theme in the shared `themes` folder, read when the dialog
    /// opens.
    themes: Vec<ThemeFile>,
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

    /// Which text uses the bundled font.
    fn font_row(&self, settings: &Settings, strings: &'static Strings) -> AnyElement {
        let current = settings.font;
        let app = self.app.clone();
        row(
            strings.setting_font,
            Button::new("font")
                .outline()
                .small()
                .label(font_label(strings, current))
                .tab_stop(false)
                .dropdown_menu(move |mut menu, _, _| {
                    for choice in FontChoice::ALL {
                        let app = app.clone();
                        menu = menu.item(
                            PopupMenuItem::new(font_label(strings, choice))
                                .checked(choice == current)
                                .on_click(move |_, window, cx| {
                                    app.update(cx, |app, cx| {
                                        app.update_settings(window, cx, move |settings| {
                                            settings.font = choice;
                                        });
                                    });
                                }),
                        );
                    }
                    menu
                }),
        )
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
        window.open_dialog(cx, move |dialog, window, _| {
            let app = app.clone();
            let file = file.clone();
            let display = display.clone();
            dialog
                .title(dialog_title(strings.delete_theme, strings.close))
                .close_button(false)
                .w(dialog_width(DialogSize::COMPACT, window))
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Theme files can be added, renamed or deleted while the dialog is
        // open, so read them again on every render.
        self.themes = persistence::list_themes().unwrap_or_default();
        let app = self.app.read(cx);
        let strings = app.strings();
        let settings = app.settings.clone();

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
                "always-show-scrollbar",
                strings.setting_always_show_scrollbar,
                settings.always_show_scrollbar,
                |settings, value| settings.always_show_scrollbar = value,
                cx,
            ),
            self.font_row(&settings, strings),
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

        let max_height = dialog_body_height(DialogSize::STANDARD, window);

        dialog_body(
            FadeScroll::new("settings-scroll", cx.theme().background)
                .max_h(max_height)
                .p_4()
                .gap_5()
                .child(section(strings.settings_timer, timer, cx))
                .child(section(strings.settings_theme, theme_rows, cx))
                .child(section(strings.settings_appearance, appearance, cx))
                .child(section(strings.settings_updates, updates, cx))
                .child(section(strings.setting_language, language, cx))
                .child(section(strings.settings_data, data, cx)),
        )
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

/// Converts a theme file color for use in the interface.
fn theme_hsla(color: ThemeColor) -> Hsla {
    rgb(color.rgb()).into()
}

/// The menu label for a font choice.
fn font_label(strings: &'static Strings, choice: FontChoice) -> &'static str {
    match choice {
        FontChoice::System => strings.font_system,
        FontChoice::Timer => strings.font_timer,
        FontChoice::Times => strings.font_times,
        FontChoice::Whole => strings.font_whole,
    }
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
            }
        });
        let strings_app = cx.entity();
        window.open_dialog(cx, move |dialog, window, cx| {
            let strings = strings_app.read(cx).strings();
            let view = view.clone();
            dialog
                .title(dialog_title(
                    div().text_lg().child(strings.settings),
                    strings.close,
                ))
                .close_button(false)
                .w(dialog_width(DialogSize::STANDARD, window))
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
