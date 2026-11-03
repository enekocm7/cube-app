//! The settings dialog.

use cube_core::Settings;
use cube_core::persistence;
use cube_core::settings::ThemePreference;
use cube_core::theme;
use cube_core::theme::{ThemeColor, ThemeFile};
use cube_locale::Language;
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

use crate::app::CubeApp;

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
}

impl SettingsView {
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

    /// One button per language, with the active one filled.
    fn language_choices(settings: &Settings, cx: &mut Context<Self>) -> Vec<Button> {
        Language::ALL
            .into_iter()
            .map(|language| {
                choice(
                    language.code(),
                    language.native_name(),
                    Language::from_code(&settings.language) == Some(language),
                )
                .on_click(cx.listener(move |view, _, window, cx| {
                    view.change(window, cx, move |settings| {
                        language.code().clone_into(&mut settings.language);
                    });
                }))
            })
            .collect()
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

        let languages = Self::language_choices(&settings, cx);
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
        let language = [h_flex()
            .min_h(ROW_MIN_HEIGHT)
            .px_3()
            .gap_2()
            .children(languages)
            .into_any_element()];

        let max_height =
            (window.viewport_size().height - DIALOG_CHROME_HEIGHT).min(BODY_MAX_HEIGHT);

        v_flex()
            .id("settings-body")
            .max_h(max_height)
            .p_4()
            .gap_5()
            .child(section(strings.settings_timer, timer, cx))
            .child(section(strings.settings_theme, theme_rows, cx))
            .child(section(strings.settings_appearance, appearance, cx))
            .child(section(strings.setting_language, language, cx))
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
            }
        });
        let strings_app = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let strings = strings_app.read(cx).strings();
            let view = view.clone();
            dialog
                .title(div().text_lg().pb_2().child(strings.settings))
                .w(px(480.))
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
