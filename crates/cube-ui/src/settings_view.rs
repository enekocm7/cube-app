//! The settings dialog.

use cube_core::Settings;
use cube_core::settings::ThemePreference;
use cube_locale::Language;
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
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.app.read(cx);
        let strings = app.strings();
        let settings = app.settings.clone();

        let theme_button = |id: &'static str,
                            label: &'static str,
                            theme: ThemePreference,
                            cx: &mut Context<Self>| {
            choice(id, label, settings.theme == theme).on_click(cx.listener(
                move |view, _, window, cx| {
                    view.change(window, cx, move |settings| settings.theme = theme);
                },
            ))
        };
        let languages = self.languages(&settings.language, cx);

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
            row(
                strings.setting_theme,
                h_flex()
                    .gap_2()
                    .child(theme_button(
                        "theme-system",
                        strings.theme_system,
                        ThemePreference::System,
                        cx,
                    ))
                    .child(theme_button(
                        "theme-light",
                        strings.theme_light,
                        ThemePreference::Light,
                        cx,
                    ))
                    .child(theme_button(
                        "theme-dark",
                        strings.theme_dark,
                        ThemePreference::Dark,
                        cx,
                    )),
            ),
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
fn choice(id: &'static str, label: &'static str, selected: bool) -> Button {
    let button = Button::new(id).small().label(label).selected(selected);
    if selected {
        button.primary()
    } else {
        button.outline()
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
        let view = cx.new(|cx| {
            cx.observe(&app, |_, _, cx| cx.notify()).detach();
            SettingsView { app: app.clone() }
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
