//! The settings dialog.

use cube_core::Settings;
use cube_core::persistence;
use cube_core::settings::{
    FontChoice, InvalidSizePt, MAX_SCRAMBLE_PT, MAX_TEXT_PT, MAX_TIMER_PT, MIN_SCRAMBLE_PT,
    MIN_TEXT_PT, MIN_TIMER_PT, ScrambleSize, TextSize, ThemePreference, TimerSize, UpdateChannel,
    UpdateFrequency,
};
use cube_core::settings::{parse_scramble_pt, parse_text_pt, parse_timer_pt};
use cube_core::theme;
use cube_core::theme::{ThemeColor, ThemeFile};
use cube_locale::{Language, Strings};
use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable as _;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{
    ActiveTheme as _, Selectable as _, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::CubeApp;
use crate::dialog_size::{DialogSize, dialog_body, dialog_body_height, dialog_title, dialog_width};
use crate::dialog_submit::{DialogSubmit as _, Submit, on_press};
use crate::scroll_fade::FadeScroll;
use crate::search_bar::{BAR_HEIGHT, focus_on_slash, new_search, query, search_bar};

const ROW_MIN_HEIGHT: Pixels = px(44.);

/// A view of the app's settings; re-renders whenever the app changes.
struct SettingsView {
    app: Entity<CubeApp>,
    /// Every valid theme in the shared `themes` folder, read when the dialog
    /// opens.
    themes: Vec<ThemeFile>,
    /// The custom scramble size in points, blank for the preset.
    scramble_pt: Entity<InputState>,
    /// Whether the text in `scramble_pt` is not a valid size.
    scramble_pt_invalid: bool,
    /// The custom text size in points, blank for the preset.
    text_pt: Entity<InputState>,
    /// Whether the text in `text_pt` is not a valid size.
    text_pt_invalid: bool,
    /// The custom timer size in points, blank for the preset.
    timer_pt: Entity<InputState>,
    /// Whether the text in `timer_pt` is not a valid size.
    timer_pt_invalid: bool,
    /// Filters the rows by label and visible value.
    search: Entity<InputState>,
    /// Holds focus so `/` reaches the dialog before anything is clicked.
    focus: FocusHandle,
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

    /// Opens a picker for the folder of the user's own backups.
    fn choose_backup_folder(&self, window: &mut Window, cx: &mut Context<Self>) {
        let prompt = self.app.read(cx).strings().backup_folder_prompt;
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(prompt.into()),
        });
        let app = self.app.clone();
        cx.spawn_in(window, async move |view, cx| match receiver.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.first() {
                    let folder = path.to_string_lossy().into_owned();
                    let _ = app.update_in(cx, |app, window, cx| {
                        app.update_settings(window, cx, |settings| {
                            settings.user_backup_dir = folder;
                        });
                    });
                    let _ = view.update(cx, |_, cx| cx.notify());
                }
            }
            Ok(Err(error)) => {
                let _ = app.update_in(cx, |app, window, cx| {
                    app.report_backup_error(&error, window, cx);
                });
            }
            // Cancelled, or the window closed while the picker was open.
            Ok(Ok(None)) | Err(_) => {}
        })
        .detach();
    }

    /// The folder of the user's own backups and the controls that change it.
    fn backup_folder_row(&self, strings: &'static Strings, cx: &mut Context<Self>) -> AnyElement {
        let folder = self.app.read(cx).settings.user_backup_dir.clone();
        let chosen = !folder.is_empty();
        let shown = if chosen {
            folder
        } else {
            strings.backup_folder_none.to_owned()
        };
        h_flex()
            .min_h(ROW_MIN_HEIGHT)
            .px_3()
            .py_2()
            .gap_4()
            .justify_between()
            .text_sm()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(strings.setting_backup_folder)
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(shown),
                    ),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_2()
                    .when(chosen, |this| {
                        this.child(
                            Button::new("backup-folder-clear")
                                .small()
                                .outline()
                                .label(strings.backup_folder_clear)
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.change(window, cx, |settings| {
                                        settings.user_backup_dir.clear();
                                    });
                                    cx.notify();
                                })),
                        )
                    })
                    .child(
                        Button::new("backup-folder-choose")
                            .small()
                            .outline()
                            .label(strings.backup_folder_button)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.choose_backup_folder(window, cx);
                            })),
                    ),
            )
            .into_any_element()
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

    /// The csTimer import and export actions and the user's backups.
    fn data_rows(&self, strings: &'static Strings, cx: &mut Context<Self>) -> [AnyElement; 4] {
        let no_folder = self.app.read(cx).settings.user_backup_dir.is_empty();
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
            self.backup_folder_row(strings, cx),
            row(
                strings.setting_backup_now,
                Button::new("backup-now")
                    .small()
                    .outline()
                    .label(strings.backup_now_button)
                    .disabled(no_folder)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.app.update(cx, |app, cx| app.back_up_now(window, cx));
                    })),
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

    /// The language choices, wrapped into short rows so sixteen languages fit.
    fn languages(&self, current: &str, cx: &mut Context<Self>) -> Vec<AnyElement> {
        Language::ALL
            .chunks(4)
            .map(|row| {
                h_flex()
                    .gap_2()
                    .children(row.iter().copied().map(|language| {
                        choice(
                            language.code(),
                            language.native_name(),
                            Language::from_code(current) == Some(language),
                        )
                        .on_click(cx.listener(
                            move |view, _, window, cx| {
                                view.change(window, cx, move |settings| {
                                    language.code().clone_into(&mut settings.language);
                                });
                            },
                        ))
                    }))
                    .into_any_element()
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
                .dropdown_caret(true)
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

    /// How large the interface text is drawn.
    fn text_size_row(&self, settings: &Settings, strings: &'static Strings) -> AnyElement {
        let current = settings.text_size;
        let app = self.app.clone();
        let input = self.text_pt.clone();
        row(
            strings.setting_text_size,
            Button::new("text-size")
                .outline()
                .small()
                .label(text_size_label(strings, current))
                .dropdown_caret(true)
                .tab_stop(false)
                .dropdown_menu(move |mut menu, _, _| {
                    for choice in TextSize::ALL {
                        let app = app.clone();
                        menu = menu.item(
                            PopupMenuItem::new(text_size_label(strings, choice))
                                .checked(choice == current)
                                .on_click({
                                    let input = input.clone();
                                    move |_, window, cx| {
                                        // Start the custom size from what is shown now.
                                        let mut start = None;
                                        app.update(cx, |app, cx| {
                                            app.update_settings(window, cx, |settings| {
                                                if choice == TextSize::Custom
                                                    && settings.text_custom_pt.is_none()
                                                {
                                                    settings.text_custom_pt =
                                                        Some(settings.text_pt());
                                                }
                                                settings.text_size = choice;
                                                start = settings.text_custom_pt;
                                            });
                                        });
                                        if let Some(points) = start {
                                            input.update(cx, |input, cx| {
                                                input.set_value(points.to_string(), window, cx);
                                            });
                                        }
                                    }
                                }),
                        );
                    }
                    menu
                }),
        )
    }

    /// How large the timer's digits are drawn.
    fn timer_size_row(&self, settings: &Settings, strings: &'static Strings) -> AnyElement {
        let current = settings.timer_size;
        let app = self.app.clone();
        let input = self.timer_pt.clone();
        row(
            strings.setting_timer_size,
            Button::new("timer-size")
                .outline()
                .small()
                .label(timer_size_label(strings, current))
                .dropdown_caret(true)
                .tab_stop(false)
                .dropdown_menu(move |mut menu, _, _| {
                    for choice in TimerSize::ALL {
                        let app = app.clone();
                        menu = menu.item(
                            PopupMenuItem::new(timer_size_label(strings, choice))
                                .checked(choice == current)
                                .on_click({
                                    let input = input.clone();
                                    move |_, window, cx| {
                                        // Start the custom size from what is shown now.
                                        let mut start = None;
                                        app.update(cx, |app, cx| {
                                            app.update_settings(window, cx, |settings| {
                                                if choice == TimerSize::Custom
                                                    && settings.timer_custom_pt.is_none()
                                                {
                                                    settings.timer_custom_pt =
                                                        Some(settings.timer_pt());
                                                }
                                                settings.timer_size = choice;
                                                start = settings.timer_custom_pt;
                                            });
                                        });
                                        if let Some(points) = start {
                                            input.update(cx, |input, cx| {
                                                input.set_value(points.to_string(), window, cx);
                                            });
                                        }
                                    }
                                }),
                        );
                    }
                    menu
                }),
        )
    }

    /// How large the scramble is drawn.
    fn scramble_size_row(&self, settings: &Settings, strings: &'static Strings) -> AnyElement {
        let current = settings.scramble_size;
        let app = self.app.clone();
        let input = self.scramble_pt.clone();
        row(
            strings.setting_scramble_size,
            Button::new("scramble-size")
                .outline()
                .small()
                .label(scramble_size_label(strings, current))
                .dropdown_caret(true)
                .tab_stop(false)
                .dropdown_menu(move |mut menu, _, _| {
                    for choice in ScrambleSize::ALL {
                        let app = app.clone();
                        menu = menu.item(
                            PopupMenuItem::new(scramble_size_label(strings, choice))
                                .checked(choice == current)
                                .on_click({
                                    let input = input.clone();
                                    move |_, window, cx| {
                                        // Start the custom size from what is shown now.
                                        let mut start = None;
                                        app.update(cx, |app, cx| {
                                            app.update_settings(window, cx, |settings| {
                                                if choice == ScrambleSize::Custom
                                                    && settings.scramble_custom_pt.is_none()
                                                {
                                                    settings.scramble_custom_pt =
                                                        Some(settings.scramble_pt());
                                                }
                                                settings.scramble_size = choice;
                                                start = settings.scramble_custom_pt;
                                            });
                                        });
                                        if let Some(points) = start {
                                            input.update(cx, |input, cx| {
                                                input.set_value(points.to_string(), window, cx);
                                            });
                                        }
                                    }
                                }),
                        );
                    }
                    menu
                }),
        )
    }

    /// A size's exact field, shown while that size is custom.
    fn custom_size_rows(
        &self,
        label: &'static str,
        input: &Entity<InputState>,
        invalid: bool,
        min: u32,
        max: u32,
        strings: &'static Strings,
    ) -> Vec<AnyElement> {
        let mut rows = vec![row(
            label,
            div().w(px(96.)).child(Input::new(input).small().w_full()),
        )];
        if invalid {
            let message = strings
                .size_custom_invalid
                .replace("{min}", &min.to_string())
                .replace("{max}", &max.to_string());
            rows.push(
                div()
                    .px_3()
                    .py_2()
                    .text_sm()
                    .text_color(gpui_kit::red())
                    .child(message)
                    .into_any_element(),
            );
        }
        rows
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
            let delete: Submit = std::rc::Rc::new(move |window, cx| {
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
            });
            dialog
                .title(dialog_title(strings.delete_theme, strings.close))
                .close_button(false)
                .w(dialog_width(DialogSize::COMPACT, window))
                .submit_on_enter(&delete)
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
                                .on_click(on_press(&delete)),
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

/// Wires a size field to its parser and settings, flagging invalid input.
fn observe_size_input(
    input: &Entity<InputState>,
    window: &mut Window,
    cx: &mut Context<SettingsView>,
    parse: fn(&str) -> Result<Option<u32>, InvalidSizePt>,
    set_value: fn(&mut Settings, Option<u32>),
    set_invalid: fn(&mut SettingsView, bool),
) {
    cx.subscribe_in(
        input,
        window,
        move |view, input, event: &InputEvent, window, cx| {
            if !matches!(event, InputEvent::Change) {
                return;
            }
            let text = input.read(cx).value().to_string();
            match parse(&text) {
                Ok(points) => {
                    set_invalid(view, false);
                    view.change(window, cx, move |settings| set_value(settings, points));
                }
                Err(InvalidSizePt) => set_invalid(view, true),
            }
            cx.notify();
        },
    )
    .detach();
}

impl Render for SettingsView {
    #[allow(clippy::too_many_lines, reason = "one tagged row list per section")]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Theme files can be added, renamed or deleted while the dialog is
        // open, so read them again on every render.
        self.themes = persistence::list_themes().unwrap_or_default();
        let app = self.app.read(cx);
        let strings = app.strings();
        let settings = app.settings.clone();

        let languages = self.languages(&settings.language, cx);
        let theme_files: Vec<String> = self
            .themes
            .iter()
            .map(|entry| theme::display_name(&entry.name).to_owned())
            .collect();
        let state = |label: &str, on: bool| {
            format!(
                "{label} {}",
                if on {
                    strings.search_on
                } else {
                    strings.search_off
                }
            )
        };

        let timer = tagged(
            [
                state(strings.setting_inspection, settings.inspection),
                state(strings.setting_inspection_audio, settings.inspection_audio),
                state(strings.setting_touch_to_start, settings.touch_to_start),
                state(strings.setting_accurate, settings.accurate_3x3),
            ],
            [
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
                    "touch-to-start",
                    strings.setting_touch_to_start,
                    settings.touch_to_start,
                    |settings, value| settings.touch_to_start = value,
                    cx,
                ),
                self.switch(
                    "accurate",
                    strings.setting_accurate,
                    settings.accurate_3x3,
                    |settings, value| settings.accurate_3x3 = value,
                    cx,
                ),
            ],
        );

        let mut theme_texts = vec![
            format!(
                "{} {} {} {}",
                strings.setting_theme,
                strings.theme_system,
                strings.theme_light,
                strings.theme_dark
            ),
            format!("{} {}", strings.custom_themes, strings.new_theme),
        ];
        let mut theme_rows = vec![
            self.theme_row(&settings, strings, cx),
            Self::new_theme_row(strings, cx),
        ];
        theme_rows.extend(self.theme_list_rows(&settings, strings, cx));
        if theme_files.is_empty() {
            theme_texts.push(strings.no_themes.to_owned());
        } else {
            theme_texts.extend(theme_files);
        }
        let themes = tagged(theme_texts, theme_rows);

        let mut font_size_texts = vec![format!(
            "{} {}",
            strings.setting_font,
            font_label(strings, settings.font)
        )];
        let mut font_size_rows = vec![self.font_row(&settings, strings)];

        font_size_texts.push(format!(
            "{} {}",
            strings.setting_text_size,
            text_size_label(strings, settings.text_size)
        ));
        font_size_rows.push(self.text_size_row(&settings, strings));
        if settings.text_size == TextSize::Custom {
            let custom = self.custom_size_rows(
                strings.setting_text_custom,
                &self.text_pt,
                self.text_pt_invalid,
                MIN_TEXT_PT,
                MAX_TEXT_PT,
                strings,
            );
            font_size_texts.extend(
                custom
                    .iter()
                    .map(|_| strings.setting_text_custom.to_owned()),
            );
            font_size_rows.extend(custom);
        }

        font_size_texts.push(format!(
            "{} {}",
            strings.setting_timer_size,
            timer_size_label(strings, settings.timer_size)
        ));
        font_size_rows.push(self.timer_size_row(&settings, strings));
        if settings.timer_size == TimerSize::Custom {
            let custom = self.custom_size_rows(
                strings.setting_timer_custom,
                &self.timer_pt,
                self.timer_pt_invalid,
                MIN_TIMER_PT,
                MAX_TIMER_PT,
                strings,
            );
            font_size_texts.extend(
                custom
                    .iter()
                    .map(|_| strings.setting_timer_custom.to_owned()),
            );
            font_size_rows.extend(custom);
        }

        font_size_texts.push(format!(
            "{} {}",
            strings.setting_scramble_size,
            scramble_size_label(strings, settings.scramble_size)
        ));
        font_size_rows.push(self.scramble_size_row(&settings, strings));
        if settings.scramble_size == ScrambleSize::Custom {
            let custom = self.custom_size_rows(
                strings.setting_scramble_custom,
                &self.scramble_pt,
                self.scramble_pt_invalid,
                MIN_SCRAMBLE_PT,
                MAX_SCRAMBLE_PT,
                strings,
            );
            font_size_texts.extend(
                custom
                    .iter()
                    .map(|_| strings.setting_scramble_custom.to_owned()),
            );
            font_size_rows.extend(custom);
        }

        let font_size = tagged(font_size_texts, font_size_rows);

        let appearance_texts = vec![
            state(strings.setting_solve_list_ao5, settings.solve_list_ao5),
            state(
                strings.setting_always_show_scrollbar,
                settings.always_show_scrollbar,
            ),
            state(strings.setting_scramble_preview, settings.scramble_preview),
            state(strings.setting_toasts, settings.toasts),
            state(strings.setting_confirm_delete, settings.confirm_delete),
        ];
        let appearance = tagged(
            appearance_texts,
            vec![
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
                self.switch(
                    "scramble-preview",
                    strings.setting_scramble_preview,
                    settings.scramble_preview,
                    |settings, value| settings.scramble_preview = value,
                    cx,
                ),
                self.switch(
                    "toasts",
                    strings.setting_toasts,
                    settings.toasts,
                    |settings, value| settings.toasts = value,
                    cx,
                ),
                self.switch(
                    "confirm-delete",
                    strings.setting_confirm_delete,
                    settings.confirm_delete,
                    |settings, value| settings.confirm_delete = value,
                    cx,
                ),
            ],
        );

        let mut update_texts = vec![
            state(strings.setting_check_updates, settings.updates.auto_check),
            format!(
                "{} {} {}",
                strings.setting_update_channel,
                strings.update_channel_stable,
                strings.update_channel_prerelease
            ),
            format!(
                "{} {} {} {}",
                strings.setting_update_frequency,
                strings.update_frequency_start,
                strings.update_frequency_daily,
                strings.update_frequency_weekly
            ),
            strings.setting_check_now.to_owned(),
        ];
        if let Some(version) = &settings.updates.skipped_version {
            update_texts.push(format!(
                "{} {}",
                strings.setting_skipping.replace("{version}", version),
                strings.setting_clear_skip
            ));
        }
        let updates = tagged(update_texts, self.update_rows(strings, &settings, cx));

        let language_text = Language::ALL
            .into_iter()
            .map(Language::native_name)
            .fold(strings.setting_language.to_owned(), |all, name| {
                format!("{all} {name}")
            });
        let language = tagged(
            [language_text],
            [v_flex()
                .min_h(ROW_MIN_HEIGHT)
                .px_3()
                .py_2()
                .gap_2()
                .children(languages)
                .into_any_element()],
        );

        let data = tagged(
            [
                format!("{} {}", strings.setting_import, strings.import_button),
                format!("{} {}", strings.setting_export, strings.export_button),
                format!(
                    "{} {}",
                    strings.setting_backup_folder, strings.backup_folder_button
                ),
                format!(
                    "{} {}",
                    strings.setting_backup_now, strings.backup_now_button
                ),
            ],
            self.data_rows(strings, cx),
        );

        let search = query(&self.search, cx);
        let sections: Vec<AnyElement> = [
            (strings.settings_timer, timer),
            (strings.settings_font_size, font_size),
            (strings.settings_theme, themes),
            (strings.settings_appearance, appearance),
            (strings.settings_updates, updates),
            (strings.setting_language, language),
            (strings.settings_data, data),
        ]
        .into_iter()
        .filter_map(|(heading, rows)| {
            // A heading that matches keeps its whole section.
            let whole = cube_core::search::matches(&search, &[heading]);
            let rows: Vec<AnyElement> = rows
                .into_iter()
                .filter(|(text, _)| whole || cube_core::search::matches(&search, &[text]))
                .map(|(_, row)| row)
                .collect();
            (!rows.is_empty()).then(|| section(heading, rows, cx).into_any_element())
        })
        .collect();
        let no_results = sections.is_empty();

        let max_height = dialog_body_height(DialogSize::STANDARD, window);

        dialog_body(
            v_flex()
                .track_focus(&self.focus)
                .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                    if focus_on_slash(event, &view.search, window, cx) {
                        cx.stop_propagation();
                    }
                }))
                .child(search_bar(&self.search, cx))
                .child(
                    FadeScroll::new("settings-scroll", cx.theme().background)
                        .max_h(max_height - BAR_HEIGHT)
                        .p_4()
                        .gap_5()
                        .children(sections)
                        .when(no_results, |column| {
                            column.child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(strings.search_no_results),
                            )
                        }),
                ),
        )
    }
}

/// Pairs each row with the text the search matches it against.
fn tagged(
    texts: impl IntoIterator<Item = String>,
    rows: impl IntoIterator<Item = AnyElement>,
) -> Vec<(String, AnyElement)> {
    texts.into_iter().zip(rows).collect()
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

/// The menu label for a text size.
fn text_size_label(strings: &'static Strings, size: TextSize) -> &'static str {
    match size {
        TextSize::Small => strings.text_size_small,
        TextSize::Normal => strings.text_size_normal,
        TextSize::Large => strings.text_size_large,
        TextSize::Largest => strings.text_size_largest,
        TextSize::Custom => strings.text_size_custom,
    }
}

/// The menu label for a timer size.
fn timer_size_label(strings: &'static Strings, size: TimerSize) -> &'static str {
    match size {
        TimerSize::Small => strings.timer_size_small,
        TimerSize::Normal => strings.timer_size_normal,
        TimerSize::Large => strings.timer_size_large,
        TimerSize::Largest => strings.timer_size_largest,
        TimerSize::Custom => strings.timer_size_custom,
    }
}

/// The menu label for a scramble size.
fn scramble_size_label(strings: &'static Strings, size: ScrambleSize) -> &'static str {
    match size {
        ScrambleSize::Small => strings.scramble_size_small,
        ScrambleSize::Normal => strings.scramble_size_normal,
        ScrambleSize::Large => strings.scramble_size_large,
        ScrambleSize::Largest => strings.scramble_size_largest,
        ScrambleSize::Custom => strings.scramble_size_custom,
    }
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
        let scramble_custom = self.settings.scramble_custom_pt;
        let text_custom = self.settings.text_custom_pt;
        let timer_custom = self.settings.timer_custom_pt;
        let strings = self.strings();
        let scramble_pt = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(scramble_custom.map_or_default(|pt| pt.to_string()))
                .placeholder("pt")
        });
        let text_pt = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(text_custom.map_or_default(|pt| pt.to_string()))
                .placeholder("pt")
        });
        let timer_pt = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(timer_custom.map_or_default(|pt| pt.to_string()))
                .placeholder("pt")
        });
        let view = cx.new(|cx| {
            cx.observe(&app, |_: &mut SettingsView, _, cx| cx.notify())
                .detach();
            let search = new_search(strings, window, cx);
            observe_size_input(
                &scramble_pt,
                window,
                cx,
                parse_scramble_pt,
                |settings, points| settings.scramble_custom_pt = points,
                |view, invalid| view.scramble_pt_invalid = invalid,
            );
            observe_size_input(
                &text_pt,
                window,
                cx,
                parse_text_pt,
                |settings, points| settings.text_custom_pt = points,
                |view, invalid| view.text_pt_invalid = invalid,
            );
            observe_size_input(
                &timer_pt,
                window,
                cx,
                parse_timer_pt,
                |settings, points| settings.timer_custom_pt = points,
                |view, invalid| view.timer_pt_invalid = invalid,
            );
            SettingsView {
                app: app.clone(),
                themes,
                scramble_pt: scramble_pt.clone(),
                scramble_pt_invalid: false,
                text_pt: text_pt.clone(),
                text_pt_invalid: false,
                timer_pt: timer_pt.clone(),
                timer_pt_invalid: false,
                search,
                focus: cx.focus_handle(),
            }
        });
        let strings_app = cx.entity();
        let first_build = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            let strings = strings_app.read(cx).strings();
            let view = view.clone();
            if first_build.replace(false) {
                let focus = view.read(cx).focus.clone();
                focus.focus(window, cx);
            }
            dialog
                .title(dialog_title(
                    div().text_lg().child(strings.settings),
                    strings.close,
                ))
                .close_button(false)
                .w(dialog_width(DialogSize::STANDARD, window))
                .close_on_enter()
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
