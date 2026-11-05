//! The theme editor, used both to create a theme and to edit one.

use std::path::PathBuf;

use cube_core::persistence;
use cube_core::theme::{
    self, ColorFormat, ThemeColor, ThemeColors, ThemeNameError, ThemeSaveError,
};
use cube_locale::Strings;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{
    ActiveTheme as _, Sizable as _, StyledExt as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::CubeApp;
use crate::dialog_size::{DialogSize, dialog_body, dialog_body_height, dialog_title, dialog_width};
use crate::dialog_submit::{DialogSubmit as _, Submit, on_press};
use crate::scroll_fade::FadeScroll;

const ROW_MIN_HEIGHT: Rems = Rems(2.75);

/// The seven colors a cube-tui theme stores.
#[derive(Clone, Copy)]
enum ColorSlot {
    Background,
    Border,
    Scramble,
    Selection,
    SelectionText,
    Text,
    Accent,
}

const SLOTS: [ColorSlot; 7] = [
    ColorSlot::Background,
    ColorSlot::Border,
    ColorSlot::Scramble,
    ColorSlot::Selection,
    ColorSlot::SelectionText,
    ColorSlot::Text,
    ColorSlot::Accent,
];

impl ColorSlot {
    fn label(self, strings: &'static Strings) -> &'static str {
        match self {
            Self::Background => strings.theme_background,
            Self::Border => strings.theme_border,
            Self::Scramble => strings.theme_scramble,
            Self::Selection => strings.theme_selection,
            Self::SelectionText => strings.theme_selection_text,
            Self::Text => strings.theme_text,
            Self::Accent => strings.theme_accent,
        }
    }

    fn get(self, colors: &ThemeColors) -> ThemeColor {
        match self {
            Self::Background => colors.background,
            Self::Border => colors.border,
            Self::Scramble => colors.scramble,
            Self::Selection => colors.selection,
            Self::SelectionText => colors.selection_text,
            Self::Text => colors.text,
            Self::Accent => colors.accent,
        }
    }

    fn set(self, colors: &mut ThemeColors, color: ThemeColor) {
        match self {
            Self::Background => colors.background = color,
            Self::Border => colors.border = color,
            Self::Scramble => colors.scramble = color,
            Self::Selection => colors.selection = color,
            Self::SelectionText => colors.selection_text = color,
            Self::Text => colors.text = color,
            Self::Accent => colors.accent = color,
        }
    }
}

/// One editable color: its value, text format and the two controls.
struct ColorDraft {
    color: ThemeColor,
    format: ColorFormat,
    input: Entity<InputState>,
    picker: Entity<ColorPickerState>,
    invalid: bool,
}

/// The editor view inside its dialog.
pub(crate) struct ThemeEditor {
    app: Entity<CubeApp>,
    dir: PathBuf,
    /// The file being edited; `None` for a new theme.
    previous: Option<String>,
    name: Entity<InputState>,
    colors: Vec<ColorDraft>,
    error: Option<SharedString>,
}

/// Everything needed to open the editor.
pub(crate) struct ThemeEditorInit {
    pub app: Entity<CubeApp>,
    pub dir: PathBuf,
    pub previous: Option<String>,
    pub name: String,
    pub colors: ThemeColors,
    pub strings: &'static Strings,
}

impl ThemeEditor {
    pub(crate) fn new(init: ThemeEditorInit, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ThemeEditorInit {
            app,
            dir,
            previous,
            name,
            colors,
            strings,
        } = init;
        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(name)
                .placeholder(strings.theme_name_placeholder)
        });
        cx.subscribe_in(&name, window, |editor, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) && editor.error.is_some() {
                editor.error = None;
                cx.notify();
            }
        })
        .detach();
        let mut drafts = Vec::with_capacity(SLOTS.len());
        for (index, slot) in SLOTS.into_iter().enumerate() {
            let color = slot.get(&colors);
            let format = ColorFormat::Hex;
            let input =
                cx.new(|cx| InputState::new(window, cx).default_value(format.format(color)));
            let picker =
                cx.new(|cx| ColorPickerState::new(window, cx).default_value(to_hsla(color)));
            cx.subscribe_in(
                &input,
                window,
                move |editor, input, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        editor.color_input_changed(index, input, window, cx);
                    }
                },
            )
            .detach();
            cx.subscribe_in(
                &picker,
                window,
                move |editor, _, event: &ColorPickerEvent, window, cx| {
                    if let ColorPickerEvent::Change(Some(color)) = event {
                        editor.color_picked(index, *color, window, cx);
                    }
                },
            )
            .detach();
            drafts.push(ColorDraft {
                color,
                format,
                input,
                picker,
                invalid: false,
            });
        }

        Self {
            app,
            dir,
            previous,
            name,
            colors: drafts,
            error: None,
        }
    }

    fn color_input_changed(
        &mut self,
        index: usize,
        input: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = input.read(cx).value();
        let draft = &mut self.colors[index];
        if let Some(color) = draft.format.parse(&text) {
            draft.color = color;
            draft.invalid = false;
            let hsla = to_hsla(color);
            draft
                .picker
                .update(cx, |picker, cx| picker.set_value(hsla, window, cx));
        } else {
            draft.invalid = true;
        }
        self.error = None;
        cx.notify();
    }

    fn color_picked(
        &mut self,
        index: usize,
        color: Hsla,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let color = from_hsla(color);
        let draft = &mut self.colors[index];
        draft.color = color;
        draft.invalid = false;
        let text = draft.format.format(color);
        draft
            .input
            .update(cx, |input, cx| input.set_value(text, window, cx));
        self.error = None;
        cx.notify();
    }

    fn change_format(
        &mut self,
        index: usize,
        format: ColorFormat,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let draft = &mut self.colors[index];
        draft.format = format;
        draft.invalid = false;
        let text = format.format(draft.color);
        draft
            .input
            .update(cx, |input, cx| input.set_value(text, window, cx));
        self.error = None;
        cx.notify();
    }

    fn theme_colors(&self) -> ThemeColors {
        let mut colors = ThemeColors::default();
        for (slot, draft) in SLOTS.iter().zip(&self.colors) {
            slot.set(&mut colors, draft.color);
        }
        colors
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.app.read(cx).strings();
        if self.colors.iter().any(|draft| draft.invalid) {
            self.error = Some(strings.theme_invalid_color.into());
            cx.notify();
            return;
        }
        let name = self.name.read(cx).value().to_string();
        let colors = self.theme_colors();
        match theme::save(&self.dir, &name, &colors, self.previous.as_deref()) {
            Ok(file_name) => {
                let display = theme::display_name(&file_name).to_owned();
                let previous = self.previous.clone();
                self.app.update(cx, |app, cx| {
                    app.theme_saved(&file_name, previous.as_deref(), &display, window, cx);
                });
                window.close_dialog(cx);
            }
            Err(ThemeSaveError::Name(error)) => {
                self.error = Some(name_error(strings, error).into());
            }
            Err(ThemeSaveError::AlreadyExists) => {
                self.error = Some(strings.theme_name_exists.into());
            }
            Err(ThemeSaveError::Failed(error)) => {
                self.error = Some(
                    strings
                        .theme_save_failed
                        .replace("{error}", &format!("{error:#}"))
                        .into(),
                );
            }
        }
        cx.notify();
    }

    fn format_button(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let entity = cx.entity().downgrade();
        let current = self.colors[index].format;
        Button::new(("theme-format", index))
            .outline()
            .small()
            .w(Rems(4.5))
            .label(current.label())
            .dropdown_caret(true)
            .dropdown_menu(move |mut menu, _, _| {
                for option in ColorFormat::ALL {
                    let entity = entity.clone();
                    menu = menu.item(
                        PopupMenuItem::new(option.label())
                            .checked(option == current)
                            .on_click(move |_, window, cx| {
                                entity
                                    .update(cx, |editor, cx| {
                                        editor.change_format(index, option, window, cx);
                                    })
                                    .ok();
                            }),
                    );
                }
                menu
            })
            .into_any_element()
    }

    fn render_color_row(
        &self,
        index: usize,
        slot: ColorSlot,
        strings: &'static Strings,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let draft = &self.colors[index];
        let theme = cx.theme();
        let text_color = if draft.invalid {
            theme.danger
        } else {
            theme.foreground
        };
        row(
            slot.label(strings),
            h_flex()
                .gap_2()
                .child(ColorPicker::new(&draft.picker).small())
                .child(self.format_button(index, cx))
                .child(
                    div().w(Rems(13.125)).child(
                        Input::new(&draft.input)
                            .small()
                            .w_full()
                            .text_color(text_color),
                    ),
                ),
        )
    }

    fn render_preview(
        &self,
        colors: &ThemeColors,
        strings: &'static Strings,
        cx: &App,
    ) -> AnyElement {
        let theme = cx.theme();
        let mono = theme.mono_font_family.clone();
        let background = to_hsla(colors.background);
        let text = to_hsla(colors.text);
        let border = to_hsla(colors.border);
        let scramble = to_hsla(colors.scramble);
        let selection = to_hsla(colors.selection);
        let selection_text = to_hsla(colors.selection_text);
        let accent = to_hsla(colors.accent);
        v_flex()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .font_semibold()
                    .text_color(theme.muted_foreground)
                    .child(strings.theme_preview),
            )
            .child(
                v_flex()
                    .gap_3()
                    .p_4()
                    .rounded(theme.radius_lg)
                    .border_1()
                    .border_color(border)
                    .bg(background)
                    .child(
                        div()
                            .font_family(mono.clone())
                            .text_color(scramble)
                            .child("R U R' U' F2 L D' B R2 U'"),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .rounded(theme.radius)
                                    .bg(selection)
                                    .text_color(selection_text)
                                    .child(strings.theme_preview_selected),
                            )
                            .child(div().text_color(accent).child("+2")),
                    )
                    .child(
                        div()
                            .font_family(mono)
                            .text_2xl()
                            .text_color(text)
                            .child("00:09.42"),
                    ),
            )
            .into_any_element()
    }
}

impl Render for ThemeEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let strings = self.app.read(cx).strings();
        let colors = self.theme_colors();
        let danger = cx.theme().danger;
        let muted = cx.theme().muted_foreground;
        let rows: Vec<AnyElement> = SLOTS
            .iter()
            .enumerate()
            .map(|(index, slot)| self.render_color_row(index, *slot, strings, cx))
            .collect();
        let max_height = dialog_body_height(DialogSize::WIDE, window);

        dialog_body(
            FadeScroll::new("theme-editor", cx.theme().background)
                .max_h(max_height)
                .p_4()
                .gap_5()
                .child(row(
                    strings.theme_name,
                    div()
                        .w(Rems(20.625))
                        .child(Input::new(&self.name).small().w_full()),
                ))
                .when_some(self.error.clone(), |body, error| {
                    body.child(div().text_sm().text_color(danger).child(error))
                })
                .child(section(strings.theme_colors, rows, cx))
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(strings.theme_alpha_hint),
                )
                .child(self.render_preview(&colors, strings, cx)),
        )
    }
}

fn name_error(strings: &'static Strings, error: ThemeNameError) -> &'static str {
    match error {
        ThemeNameError::Empty => strings.theme_name_empty,
        ThemeNameError::Invalid => strings.theme_name_invalid,
        ThemeNameError::TooLong => strings.theme_name_too_long,
    }
}

fn to_hsla(color: ThemeColor) -> Hsla {
    rgb(color.rgb()).into()
}

fn from_hsla(color: Hsla) -> ThemeColor {
    let rgba: Rgba = color.into();
    ThemeColor::new(channel(rgba.r), channel(rgba.g), channel(rgba.b))
}

fn channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
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
    pub(crate) fn open_theme_editor(
        &self,
        file: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let strings = self.strings();
        let (name, colors) = if let Some(file) = &file {
            match persistence::load_theme(file) {
                Ok(colors) => (theme::display_name(file).to_owned(), colors),
                Err(error) => {
                    let message = strings
                        .theme_load_failed
                        .replace("{error}", &format!("{error:#}"));
                    self.toast(&message, window, cx);
                    return;
                }
            }
        } else {
            let colors = if self.settings.theme_file.trim().is_empty() {
                ThemeColors::default()
            } else {
                persistence::load_theme(&self.settings.theme_file).unwrap_or_default()
            };
            (String::new(), colors)
        };
        let app = cx.entity();
        let dir = match persistence::themes_dir() {
            Ok(dir) => dir,
            Err(error) => {
                let message = strings
                    .theme_load_failed
                    .replace("{error}", &format!("{error:#}"));
                self.toast(&message, window, cx);
                return;
            }
        };
        let is_new = file.is_none();
        let view = cx.new(|cx| {
            ThemeEditor::new(
                ThemeEditorInit {
                    app,
                    dir,
                    previous: file,
                    name,
                    colors,
                    strings,
                },
                window,
                cx,
            )
        });
        let strings_app = cx.entity();
        let content_view = view.clone();
        let save_view = view.clone();
        window.open_dialog(cx, move |dialog, window, cx| {
            let strings = strings_app.read(cx).strings();
            let content_view = content_view.clone();
            let save_view = save_view.clone();
            let save: Submit = std::rc::Rc::new(move |window, cx| {
                save_view.update(cx, |editor, cx| editor.save(window, cx));
            });
            dialog
                .title(dialog_title(
                    div().text_lg().child(if is_new {
                        strings.new_theme
                    } else {
                        strings.edit_theme
                    }),
                    strings.close,
                ))
                .close_button(false)
                .w(dialog_width(DialogSize::WIDE, window))
                .submit_on_enter(&save)
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
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("theme-editor-cancel")
                                .outline()
                                .label(strings.cancel)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("theme-editor-save")
                                .primary()
                                .label(strings.save)
                                .on_click(on_press(&save)),
                        ),
                )
        });
    }

    /// Applies a saved theme when it is the one on screen, then reports it.
    pub(crate) fn theme_saved(
        &mut self,
        file_name: &str,
        previous: Option<&str>,
        display: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let active =
            previous.is_none_or(|previous| self.settings.theme_file.eq_ignore_ascii_case(previous));
        if active {
            self.update_settings(window, cx, |settings| {
                file_name.clone_into(&mut settings.theme_file);
            });
        }
        let message = self.strings().theme_saved.replace("{name}", display);
        self.toast(&message, window, cx);
        cx.notify();
    }

    /// Clears the active theme when it is the one being deleted, then reports it.
    pub(crate) fn theme_deleted(
        &mut self,
        file_name: &str,
        display: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings.theme_file.eq_ignore_ascii_case(file_name) {
            self.update_settings(window, cx, |settings| settings.theme_file.clear());
        }
        let message = self.strings().theme_deleted.replace("{name}", display);
        self.toast(&message, window, cx);
        cx.notify();
    }
}
