//! The settings dialog.

use cube_core::Settings;
use cube_core::settings::ThemePreference;
use cube_locale::Language;
use gpui_kit::component::button::Button;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Selectable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::*;

use crate::app::CubeApp;

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
    ) -> impl IntoElement {
        h_flex().justify_between().gap_4().child(label).child(
            Switch::new(id).checked(checked).on_click(cx.listener(
                move |view, value: &bool, window, cx| {
                    let value = *value;
                    view.change(window, cx, move |settings| set(settings, value));
                },
            )),
        )
    }
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.app.read(cx);
        let strings = app.strings();
        let settings = app.settings.clone();
        let muted = cx.theme().muted_foreground;

        let theme_button = |id: &'static str,
                            label: &'static str,
                            theme: ThemePreference,
                            cx: &mut Context<Self>| {
            Button::new(id)
                .outline()
                .label(label)
                .selected(settings.theme == theme)
                .on_click(cx.listener(move |view, _, window, cx| {
                    view.change(window, cx, move |settings| settings.theme = theme);
                }))
        };
        let languages: Vec<Button> = Language::ALL
            .into_iter()
            .map(|language| {
                Button::new(language.code())
                    .outline()
                    .label(language.native_name())
                    .selected(Language::from_code(&settings.language) == Some(language))
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.change(window, cx, move |settings| {
                            language.code().clone_into(&mut settings.language);
                        });
                    }))
            })
            .collect();

        v_flex()
            .gap_4()
            .child(self.switch(
                "inspection",
                strings.setting_inspection,
                settings.inspection,
                |settings, value| settings.inspection = value,
                cx,
            ))
            .child(self.switch(
                "toasts",
                strings.setting_toasts,
                settings.toasts,
                |settings, value| settings.toasts = value,
                cx,
            ))
            .child(self.switch(
                "accurate",
                strings.setting_accurate,
                settings.accurate_3x3,
                |settings, value| settings.accurate_3x3 = value,
                cx,
            ))
            .child(self.switch(
                "solve-list-ao5",
                strings.setting_solve_list_ao5,
                settings.solve_list_ao5,
                |settings, value| settings.solve_list_ao5 = value,
                cx,
            ))
            .child(section(
                strings.setting_theme,
                muted,
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
            ))
            .child(section(
                strings.setting_language,
                muted,
                h_flex().gap_2().children(languages),
            ))
    }
}

/// A small muted label above its controls.
fn section(label: &'static str, color: Hsla, content: impl IntoElement) -> impl IntoElement {
    v_flex()
        .gap_2()
        .child(div().text_xs().text_color(color).child(label))
        .child(content)
}

impl CubeApp {
    pub(crate) fn open_settings(&self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity();
        let view = cx.new(|cx| {
            cx.observe(&app, |_, _, cx| cx.notify()).detach();
            SettingsView { app: app.clone() }
        });
        let title_app = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let title = title_app.read(cx).strings().settings;
            dialog.title(title).w(px(460.)).child(view.clone())
        });
    }
}
