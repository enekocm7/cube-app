//! The settings dialog: a category list on the left and the settings of the
//! selected category on the right.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use cube_core::dialogs::{reveal_shift, settings_nav_width};
use cube_core::persistence;
use cube_core::settings::{
    DEFAULT_BEST_MIN_SOLVES, MAX_BEST_MIN_SOLVES, parse_best_min_solves, parse_scramble_pt,
    parse_text_pt, parse_timer_pt,
};
use cube_core::settings::{
    FontChoice, InvalidSizePt, MAX_SCRAMBLE_PT, MAX_TEXT_PT, MAX_TIMER_PT, MIN_SCRAMBLE_PT,
    MIN_TEXT_PT, MIN_TIMER_PT, PreviewSize, ScrambleSize, TextSize, ThemePreference, TimerSize,
    UpdateChannel, UpdateFrequency,
};
use cube_core::theme;
use cube_core::theme::{ThemeColor, ThemeFile};
use cube_core::{Settings, SettingsCategory, WcaEvent};
use cube_locale::{Language, Strings};
use gpui_kit::assets::IconName;
use gpui_kit::base::{Disableable as _, ElementExt as _};
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Selectable as _, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::CubeApp;
use crate::dialog_size::{
    DialogSize, dialog_body_height, dialog_close_button, dialog_title, dialog_width, text_scale,
};
use crate::dialog_submit::{DialogSubmit as _, Submit, on_press};
use crate::scroll_fade::FadeScroll;
use crate::search_bar::{focus_on_slash, new_search, query, search_bar};

const ROW_MIN_HEIGHT: Rems = Rems(3.5);
const HEADER_HEIGHT: Rems = Rems(3.25);
/// The tallest the language menu gets before it scrolls.
const LANGUAGE_MENU_HEIGHT: Pixels = px(320.);
/// Room kept between a row reached with the keyboard and the pane's edges, so
/// the row is not left under an edge fade.
const REVEAL_MARGIN: f32 = 28.;

/// Chooses a preset for an event and returns the custom points it is left with.
type ApplySize = Rc<dyn Fn(&mut Settings) -> Option<u32>>;

/// One preset of an event's size menu. `apply` chooses it for the event and
/// returns the custom points the event is left with.
struct SizeOption {
    label: &'static str,
    selected: bool,
    apply: ApplySize,
}

/// A size an event can set for itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum EventSizeKind {
    Scramble,
    Timer,
}

/// A view of the app's settings; re-renders whenever the app changes.
#[allow(
    clippy::struct_excessive_bools,
    reason = "one invalid flag per number field"
)]
struct SettingsView {
    app: Entity<CubeApp>,
    /// The custom size field of each event that can have its own size.
    event_pts: BTreeMap<(EventSizeKind, WcaEvent), Entity<InputState>>,
    /// The event size fields whose text is not a valid size.
    event_pt_invalid: BTreeSet<(EventSizeKind, WcaEvent)>,
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
    /// The earlier solves a best needs, blank for the default.
    best_min_solves: Entity<InputState>,
    /// Whether the text in `best_min_solves` is not a valid count.
    best_min_solves_invalid: bool,
    /// Filters the rows of every category by label and visible value.
    search: Entity<InputState>,
    /// Holds focus after a click on the dialog's surface, so `/` still reaches
    /// it.
    focus: FocusHandle,
    /// The category shown while nothing is being searched.
    category: SettingsCategory,
    /// The category list's one tab stop. Up and Down move inside the list, so
    /// Tab goes straight on to the settings.
    categories_focus: FocusHandle,
    /// The scroll position of the settings pane.
    pane_scroll: ScrollHandle,
    /// One handle per row on screen, to tell which row holds focus.
    row_focus: Vec<FocusHandle>,
    /// The row last scrolled into view, so the wheel can move away from it.
    revealed_row: Rc<Cell<Option<usize>>>,
}

/// A titled card of rows in one category, each with the text the search matches
/// it against.
struct Section {
    category: SettingsCategory,
    heading: &'static str,
    rows: Vec<(String, AnyElement)>,
}

impl SettingsView {
    /// Shows `category` from its top, leaving any search.
    fn select_category(
        &mut self,
        category: SettingsCategory,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.category = category;
        self.pane_scroll.set_offset(Point::default());
        self.revealed_row.set(None);
        if !query(&self.search, cx).is_empty() {
            self.search
                .update(cx, |search, cx| search.set_value("", window, cx));
        }
        cx.notify();
    }

    /// Moves through the category list from the keyboard.
    fn on_categories_key(
        &mut self,
        event: &KeyDownEvent,
        listed: &[SettingsCategory],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.shift || modifiers.platform {
            return;
        }
        let target = match event.keystroke.key.as_str() {
            "up" => self.category.step_in(listed, -1),
            "down" => self.category.step_in(listed, 1),
            "home" => listed.first().copied(),
            "end" => listed.last().copied(),
            "right" | "enter" | "space" => {
                window.focus_next(cx);
                cx.stop_propagation();
                return;
            }
            _ => return,
        };
        if let Some(category) = target {
            self.select_category(category, window, cx);
        }
        cx.stop_propagation();
    }

    /// Goes back to the category list with Left, unless a text field needs the
    /// key.
    fn on_pane_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        let plain = !modifiers.control && !modifiers.alt && !modifiers.shift && !modifiers.platform;
        if plain && event.keystroke.key == "left" && !window.has_focused_input(cx) {
            self.categories_focus.focus(window, cx);
            cx.stop_propagation();
        }
    }

    /// The category list, showing only the categories with matches during a
    /// search.
    fn render_categories(
        &self,
        listed: &[SettingsCategory],
        searching: bool,
        strings: &'static Strings,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let (hover, active, ring) = (theme.sidebar_accent, theme.secondary, theme.ring);
        let (background, foreground) = (theme.sidebar, theme.sidebar_foreground);
        let focused_list = self.categories_focus.is_focused(window);
        let keys: Rc<[SettingsCategory]> = listed.into();
        FadeScroll::new("settings-categories", background)
            .flex_1()
            .min_h_0()
            .px_2()
            .pb_2()
            .gap_0p5()
            .track_focus(&self.categories_focus)
            .on_key_down(cx.listener(move |view, event: &KeyDownEvent, window, cx| {
                view.on_categories_key(event, &keys, window, cx);
            }))
            .children(listed.iter().copied().enumerate().map(|(index, category)| {
                let selected = !searching && category == self.category;
                h_flex()
                    .id(("settings-category", index))
                    .h(Rems(2.25))
                    .flex_none()
                    .px_2()
                    .gap_2()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(transparent_black())
                    .text_sm()
                    .text_color(foreground)
                    .cursor_pointer()
                    .hover(move |row| row.bg(hover))
                    .when(selected, move |row| row.bg(active).font_medium())
                    .when(selected && focused_list, move |row| row.border_color(ring))
                    .child(Icon::new(category_icon(category)).flex_none())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(category_label(strings, category)),
                    )
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.select_category(category, window, cx);
                    }))
            }))
    }

    /// A row's own focus handle, created the first time it is on screen.
    fn row_focus(&mut self, index: usize, cx: &mut Context<Self>) -> FocusHandle {
        while self.row_focus.len() <= index {
            self.row_focus.push(cx.focus_handle());
        }
        self.row_focus[index].clone()
    }

    /// Scrolls `row` into view once when focus moves into it, as gets a control
    /// below the fold on screen when Tab reaches it.
    fn reveal_on_focus(
        &self,
        index: usize,
        focus: FocusHandle,
        row: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        let scroll = self.pane_scroll.clone();
        let revealed = Rc::clone(&self.revealed_row);
        let view = cx.weak_entity();
        row.relative()
            .track_focus(&focus)
            .on_prepaint(move |bounds, window, cx| {
                if !focus.contains_focused(window, cx)
                    || revealed.replace(Some(index)) == Some(index)
                {
                    return;
                }
                let region = scroll.bounds();
                let shift = reveal_shift(
                    (f32::from(bounds.top()), f32::from(bounds.bottom())),
                    (f32::from(region.top()), f32::from(region.bottom())),
                    REVEAL_MARGIN,
                );
                if shift != 0. {
                    let offset = scroll.offset();
                    scroll.set_offset(point(offset.x, offset.y + px(shift)));
                    window.on_next_frame(move |_, cx| {
                        let _ = view.update(cx, |_, cx| cx.notify());
                    });
                }
            })
    }

    /// A heading above a card of rows, split by hairlines. `first_row` numbers
    /// the rows across the whole pane.
    fn section(
        &mut self,
        heading: &'static str,
        rows: Vec<AnyElement>,
        first_row: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let border = cx.theme().border;
        let radius = cx.theme().radius_lg;
        let rows: Vec<AnyElement> = rows
            .into_iter()
            .enumerate()
            .map(|(index, row)| {
                let focus = self.row_focus(first_row + index, cx);
                let row = div()
                    .when(index > 0, |row| row.border_t_1().border_color(border))
                    .child(row);
                self.reveal_on_focus(first_row + index, focus, row, cx)
                    .into_any_element()
            })
            .collect();
        v_flex()
            .gap_2()
            .child(div().px_1().text_base().font_semibold().child(heading))
            .child(
                v_flex()
                    .rounded(radius)
                    .border_1()
                    .border_color(border)
                    .children(rows),
            )
            .into_any_element()
    }

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
        let clear = chosen.then(|| {
            Button::new("backup-folder-clear")
                .small()
                .outline()
                .label(strings.backup_folder_clear)
                .on_click(cx.listener(|view, _, window, cx| {
                    view.change(window, cx, |settings| {
                        settings.user_backup_dir.clear();
                    });
                    cx.notify();
                }))
        });
        let choose = Button::new("backup-folder-choose")
            .small()
            .outline()
            .label(strings.backup_folder_button)
            .on_click(cx.listener(|view, _, window, cx| {
                view.choose_backup_folder(window, cx);
            }));
        row(
            strings.setting_backup_folder,
            shown,
            h_flex().gap_2().children(clear).child(choose),
            cx,
        )
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
    fn transfer_rows(strings: &'static Strings, cx: &mut Context<Self>) -> [AnyElement; 2] {
        [
            row(
                strings.setting_import,
                strings.setting_import_desc,
                Button::new("import-cstimer")
                    .small()
                    .outline()
                    .label(strings.import_button)
                    .on_click(cx.listener(|view, _, window, cx| view.import(window, cx))),
                cx,
            ),
            row(
                strings.setting_export,
                strings.setting_export_desc,
                Button::new("export-cstimer")
                    .small()
                    .outline()
                    .label(strings.export_button)
                    .on_click(cx.listener(|view, _, window, cx| view.export(window, cx))),
                cx,
            ),
        ]
    }

    /// The folder of the user's own backups and the backup they can start.
    fn backup_rows(&self, strings: &'static Strings, cx: &mut Context<Self>) -> [AnyElement; 2] {
        let no_folder = self.app.read(cx).settings.user_backup_dir.is_empty();
        [
            self.backup_folder_row(strings, cx),
            row(
                strings.setting_backup_now,
                strings.setting_backup_now_desc,
                Button::new("backup-now")
                    .small()
                    .outline()
                    .label(strings.backup_now_button)
                    .disabled(no_folder)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.app.update(cx, |app, cx| app.back_up_now(window, cx));
                    })),
                cx,
            ),
        ]
    }

    fn switch(
        &self,
        id: &'static str,
        label: &'static str,
        description: &'static str,
        checked: bool,
        set: fn(&mut Settings, bool),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        row(
            label,
            description,
            Switch::new(id).checked(checked).on_click(cx.listener(
                move |view, value: &bool, window, cx| {
                    let value = *value;
                    view.change(window, cx, move |settings| set(settings, value));
                },
            )),
            cx,
        )
    }

    /// The interface language. Sixteen choices, so the menu scrolls.
    fn language_row(&self, strings: &'static Strings, cx: &App) -> AnyElement {
        let current = self.app.read(cx).language();
        let app = self.app.clone();
        row(
            strings.setting_language,
            strings.setting_language_desc,
            Button::new("language")
                .outline()
                .small()
                .label(current.native_name())
                .dropdown_caret(true)
                .dropdown_menu(move |mut menu, _, _| {
                    menu = menu.scrollable(true).max_h(LANGUAGE_MENU_HEIGHT);
                    for language in Language::ALL {
                        let app = app.clone();
                        menu = menu.item(
                            PopupMenuItem::new(language.native_name())
                                .checked(current == language)
                                .on_click(move |_, window, cx| {
                                    app.update(cx, |app, cx| {
                                        app.update_settings(window, cx, move |settings| {
                                            language.code().clone_into(&mut settings.language);
                                        });
                                    });
                                }),
                        );
                    }
                    menu
                }),
            cx,
        )
    }

    /// Which text uses the bundled font.
    fn font_row(&self, settings: &Settings, strings: &'static Strings, cx: &App) -> AnyElement {
        let current = settings.font;
        let app = self.app.clone();
        row(
            strings.setting_font,
            strings.setting_font_desc,
            Button::new("font")
                .outline()
                .small()
                .label(font_label(strings, current))
                .dropdown_caret(true)
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
            cx,
        )
    }

    /// How large the interface text is drawn.
    fn text_size_row(
        &self,
        settings: &Settings,
        strings: &'static Strings,
        cx: &App,
    ) -> AnyElement {
        let current = settings.text_size;
        let app = self.app.clone();
        let input = self.text_pt.clone();
        row(
            strings.setting_text_size,
            strings.setting_text_size_desc,
            Button::new("text-size")
                .outline()
                .small()
                .label(text_size_label(strings, current))
                .dropdown_caret(true)
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
            cx,
        )
    }

    /// How large the timer's digits are drawn.
    fn timer_size_row(
        &self,
        settings: &Settings,
        strings: &'static Strings,
        cx: &App,
    ) -> AnyElement {
        let current = settings.timer_size;
        let app = self.app.clone();
        let input = self.timer_pt.clone();
        row(
            strings.setting_timer_size,
            strings.setting_timer_size_desc,
            Button::new("timer-size")
                .outline()
                .small()
                .label(timer_size_label(strings, current))
                .dropdown_caret(true)
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
            cx,
        )
    }

    /// How large the scramble is drawn.
    fn scramble_size_row(
        &self,
        settings: &Settings,
        strings: &'static Strings,
        cx: &App,
    ) -> AnyElement {
        let current = settings.scramble_size;
        let app = self.app.clone();
        let input = self.scramble_pt.clone();
        row(
            strings.setting_scramble_size,
            strings.setting_scramble_size_desc,
            Button::new("scramble-size")
                .outline()
                .small()
                .label(scramble_size_label(strings, current))
                .dropdown_caret(true)
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
            cx,
        )
    }

    /// The rows of one event's own size of `kind`: a menu of the presets and,
    /// while it is custom, the exact field. Each event's rows are laid out like
    /// the preview size's.
    fn event_size_rows(
        &self,
        kind: EventSizeKind,
        event: WcaEvent,
        settings: &Settings,
        strings: &'static Strings,
        cx: &App,
    ) -> Vec<AnyElement> {
        let (title, description, custom_title, range) = match kind {
            EventSizeKind::Scramble => (
                strings.setting_scramble_size_event,
                strings.setting_scramble_size_event_desc,
                strings.setting_scramble_custom_event,
                (MIN_SCRAMBLE_PT, MAX_SCRAMBLE_PT),
            ),
            EventSizeKind::Timer => (
                strings.setting_timer_size_event,
                strings.setting_timer_size_event_desc,
                strings.setting_timer_custom_event,
                (MIN_TIMER_PT, MAX_TIMER_PT),
            ),
        };
        let title = title.replace("{event}", event.name());
        let (options, custom): (Vec<SizeOption>, bool) = match kind {
            EventSizeKind::Scramble => {
                let current = settings.scramble_choice(event).size;
                let options = ScrambleSize::ALL
                    .into_iter()
                    .map(|choice| SizeOption {
                        label: scramble_size_label(strings, choice),
                        selected: choice == current,
                        apply: Rc::new(move |settings| {
                            settings.set_scramble_size(event, choice);
                            settings.scramble_choice(event).custom_pt
                        }),
                    })
                    .collect();
                (options, current == ScrambleSize::Custom)
            }
            EventSizeKind::Timer => {
                let current = settings.timer_choice(event).size;
                let options = TimerSize::ALL
                    .into_iter()
                    .map(|choice| SizeOption {
                        label: timer_size_label(strings, choice),
                        selected: choice == current,
                        apply: Rc::new(move |settings| {
                            settings.set_timer_size(event, choice);
                            settings.timer_choice(event).custom_pt
                        }),
                    })
                    .collect();
                (options, current == TimerSize::Custom)
            }
        };
        let label = options
            .iter()
            .find(|option| option.selected)
            .map_or("", |option| option.label);
        let app = self.app.clone();
        let input = self.event_pts[&(kind, event)].clone();
        let mut rows = vec![row(
            title,
            description,
            Button::new(SharedString::from(format!("event-size-{kind:?}-{event:?}")))
                .outline()
                .small()
                .label(label)
                .dropdown_caret(true)
                .dropdown_menu(move |mut menu, _, _| {
                    for option in &options {
                        let app = app.clone();
                        let input = input.clone();
                        let apply = option.apply.clone();
                        menu = menu.item(
                            PopupMenuItem::new(option.label)
                                .checked(option.selected)
                                .on_click(move |_, window, cx| {
                                    let mut start = None;
                                    app.update(cx, |app, cx| {
                                        app.update_settings(window, cx, |settings| {
                                            start = apply(settings);
                                        });
                                    });
                                    // Custom starts from what is shown now.
                                    if let Some(points) = start {
                                        input.update(cx, |input, cx| {
                                            input.set_value(points.to_string(), window, cx);
                                        });
                                    }
                                }),
                        );
                    }
                    menu
                }),
            cx,
        )];
        if custom {
            rows.extend(number_rows(
                custom_title.replace("{event}", event.name()),
                strings.size_custom_range,
                &self.event_pts[&(kind, event)],
                self.event_pt_invalid.contains(&(kind, event)),
                range,
                strings,
                cx,
            ));
        }
        rows
    }

    /// How large the scramble preview is drawn for one event, listing only the
    /// sizes the event offers.
    fn preview_size_row(
        &self,
        event: WcaEvent,
        settings: &Settings,
        strings: &'static Strings,
        cx: &App,
    ) -> AnyElement {
        let current = settings.preview_size(event);
        let app = self.app.clone();
        row(
            preview_size_title(strings, event),
            strings.setting_preview_size_desc,
            Button::new(SharedString::from(format!("preview-size-{event:?}")))
                .outline()
                .small()
                .label(preview_size_label(strings, current))
                .dropdown_caret(true)
                .dropdown_menu(move |mut menu, _, _| {
                    for choice in PreviewSize::offered_for(event) {
                        let app = app.clone();
                        menu = menu.item(
                            PopupMenuItem::new(preview_size_label(strings, choice))
                                .checked(choice == current)
                                .on_click(move |_, window, cx| {
                                    app.update(cx, |app, cx| {
                                        app.update_settings(window, cx, |settings| {
                                            settings.set_preview_size(event, choice);
                                        });
                                    });
                                }),
                        );
                    }
                    menu
                }),
            cx,
        )
    }

    /// A size's exact field, shown while that size is custom.
    fn custom_size_rows(
        &self,
        label: &'static str,
        input: &Entity<InputState>,
        invalid: bool,
        (min, max): (u32, u32),
        strings: &'static Strings,
        cx: &App,
    ) -> Vec<AnyElement> {
        number_rows(
            label,
            strings.size_custom_range,
            input,
            invalid,
            (min, max),
            strings,
            cx,
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
                strings.setting_check_updates_desc,
                settings.updates.auto_check,
                |settings, value| settings.updates.auto_check = value,
                cx,
            ),
            row(
                strings.setting_update_channel,
                strings.setting_update_channel_desc,
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
                cx,
            ),
            row(
                strings.setting_update_frequency,
                strings.setting_update_frequency_desc,
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
                cx,
            ),
            row(
                strings.setting_check_now,
                strings.setting_check_now_desc,
                Button::new("check-updates")
                    .small()
                    .outline()
                    .label(strings.setting_check_now)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.app
                            .update(cx, |app, cx| app.check_for_updates(true, window, cx));
                    })),
                cx,
            ),
        ];
        if let Some(version) = settings.updates.skipped_version.clone() {
            rows.push(row(
                strings.setting_skipping.replace("{version}", &version),
                "",
                Button::new("clear-skip")
                    .small()
                    .outline()
                    .label(strings.setting_clear_skip)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.change(window, cx, |settings| {
                            settings.updates.skipped_version = None;
                        });
                    })),
                cx,
            ));
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
                    .px_4()
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
                    .min_h(Rems(2.75))
                    .px_4()
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
            strings.custom_themes_desc,
            Button::new("theme-new")
                .primary()
                .small()
                .label(strings.new_theme)
                .on_click(cx.listener(|view, _, window, cx| {
                    view.app.update(cx, |app, cx| {
                        app.open_theme_editor(None, window, cx);
                    });
                })),
            cx,
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
            strings.setting_theme_desc,
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
            cx,
        )
    }

    /// Every section of every category, in the order they are shown.
    #[allow(clippy::too_many_lines, reason = "one tagged row list per section")]
    fn sections(&self, settings: &Settings, cx: &mut Context<Self>) -> Vec<Section> {
        use SettingsCategory as Category;

        let strings = self.app.read(cx).strings();
        let state = |label: &str, description: &str, on: bool| {
            let value = if on {
                strings.search_on
            } else {
                strings.search_off
            };
            format!("{label} {description} {value}")
        };
        let text =
            |label: &str, description: &str, value: &str| format!("{label} {description} {value}");

        let language_text = Language::ALL.into_iter().map(Language::native_name).fold(
            text(strings.setting_language, strings.setting_language_desc, ""),
            |all, name| format!("{all} {name}"),
        );
        let language = tagged([language_text], [self.language_row(strings, cx)]);

        let notifications = tagged(
            [
                state(
                    strings.setting_toasts,
                    strings.setting_toasts_desc,
                    settings.toasts,
                ),
                state(
                    strings.setting_confirm_delete,
                    strings.setting_confirm_delete_desc,
                    settings.confirm_delete,
                ),
            ],
            [
                self.switch(
                    "toasts",
                    strings.setting_toasts,
                    strings.setting_toasts_desc,
                    settings.toasts,
                    |settings, value| settings.toasts = value,
                    cx,
                ),
                self.switch(
                    "confirm-delete",
                    strings.setting_confirm_delete,
                    strings.setting_confirm_delete_desc,
                    settings.confirm_delete,
                    |settings, value| settings.confirm_delete = value,
                    cx,
                ),
            ],
        );

        let mut timer = tagged(
            [
                state(
                    strings.setting_inspection,
                    strings.setting_inspection_desc,
                    settings.inspection,
                ),
                state(
                    strings.setting_inspection_audio,
                    strings.setting_inspection_audio_desc,
                    settings.inspection_audio,
                ),
                state(
                    strings.setting_touch_to_start,
                    strings.setting_touch_to_start_desc,
                    settings.touch_to_start,
                ),
            ],
            [
                self.switch(
                    "inspection",
                    strings.setting_inspection,
                    strings.setting_inspection_desc,
                    settings.inspection,
                    |settings, value| settings.inspection = value,
                    cx,
                ),
                self.switch(
                    "inspection-audio",
                    strings.setting_inspection_audio,
                    strings.setting_inspection_audio_desc,
                    settings.inspection_audio,
                    |settings, value| settings.inspection_audio = value,
                    cx,
                ),
                self.switch(
                    "touch-to-start",
                    strings.setting_touch_to_start,
                    strings.setting_touch_to_start_desc,
                    settings.touch_to_start,
                    |settings, value| settings.touch_to_start = value,
                    cx,
                ),
            ],
        );
        timer.extend(tagged(
            [
                state(
                    strings.setting_celebrate,
                    strings.setting_celebrate_desc,
                    settings.celebrate_bests,
                ),
                state(
                    strings.setting_celebrate_sound,
                    strings.setting_celebrate_sound_desc,
                    settings.celebrate_sound,
                ),
            ],
            [
                self.switch(
                    "celebrate-bests",
                    strings.setting_celebrate,
                    strings.setting_celebrate_desc,
                    settings.celebrate_bests,
                    |settings, value| settings.celebrate_bests = value,
                    cx,
                ),
                self.switch(
                    "celebrate-sound",
                    strings.setting_celebrate_sound,
                    strings.setting_celebrate_sound_desc,
                    settings.celebrate_sound,
                    |settings, value| settings.celebrate_sound = value,
                    cx,
                ),
            ],
        ));
        let best_min_text = text(
            strings.setting_best_min_solves,
            strings.setting_best_min_solves_desc,
            &settings.best_min_solves.to_string(),
        );
        timer.extend(
            number_rows(
                strings.setting_best_min_solves,
                strings.setting_best_min_solves_desc,
                &self.best_min_solves,
                self.best_min_solves_invalid,
                (0, MAX_BEST_MIN_SOLVES),
                strings,
                cx,
            )
            .into_iter()
            .map(|row| (best_min_text.clone(), row)),
        );
        timer.extend(tagged(
            [state(
                strings.setting_reduce_motion,
                strings.setting_reduce_motion_desc,
                settings.reduce_motion,
            )],
            [self.switch(
                "reduce-motion",
                strings.setting_reduce_motion,
                strings.setting_reduce_motion_desc,
                settings.reduce_motion,
                |settings, value| settings.reduce_motion = value,
                cx,
            )],
        ));

        let scrambles = tagged(
            [
                state(
                    strings.setting_accurate,
                    strings.setting_accurate_desc,
                    settings.accurate_3x3,
                ),
                state(
                    strings.setting_scramble_preview,
                    strings.setting_scramble_preview_desc,
                    settings.scramble_preview,
                ),
            ]
            .into_iter()
            .chain(WcaEvent::ALL.into_iter().map(|event| {
                text(
                    &preview_size_title(strings, event),
                    strings.setting_preview_size_desc,
                    preview_size_label(strings, settings.preview_size(event)),
                )
            })),
            [
                self.switch(
                    "accurate",
                    strings.setting_accurate,
                    strings.setting_accurate_desc,
                    settings.accurate_3x3,
                    |settings, value| settings.accurate_3x3 = value,
                    cx,
                ),
                self.switch(
                    "scramble-preview",
                    strings.setting_scramble_preview,
                    strings.setting_scramble_preview_desc,
                    settings.scramble_preview,
                    |settings, value| settings.scramble_preview = value,
                    cx,
                ),
            ]
            .into_iter()
            .chain(
                WcaEvent::ALL
                    .into_iter()
                    .map(|event| self.preview_size_row(event, settings, strings, cx)),
            ),
        );

        let mut theme_texts = vec![
            format!(
                "{} {} {} {} {}",
                strings.setting_theme,
                strings.setting_theme_desc,
                strings.theme_system,
                strings.theme_light,
                strings.theme_dark
            ),
            text(
                strings.custom_themes,
                strings.custom_themes_desc,
                strings.new_theme,
            ),
        ];
        let mut theme_rows = vec![
            self.theme_row(settings, strings, cx),
            Self::new_theme_row(strings, cx),
        ];
        theme_rows.extend(self.theme_list_rows(settings, strings, cx));
        if self.themes.is_empty() {
            theme_texts.push(strings.no_themes.to_owned());
        } else {
            theme_texts.extend(
                self.themes
                    .iter()
                    .map(|entry| theme::display_name(&entry.name).to_owned()),
            );
        }
        let themes = tagged(theme_texts, theme_rows);

        let mut font_size_texts = vec![text(
            strings.setting_font,
            strings.setting_font_desc,
            font_label(strings, settings.font),
        )];
        let mut font_size_rows = vec![self.font_row(settings, strings, cx)];

        font_size_texts.push(text(
            strings.setting_text_size,
            strings.setting_text_size_desc,
            text_size_label(strings, settings.text_size),
        ));
        font_size_rows.push(self.text_size_row(settings, strings, cx));
        if settings.text_size == TextSize::Custom {
            let custom = self.custom_size_rows(
                strings.setting_text_custom,
                &self.text_pt,
                self.text_pt_invalid,
                (MIN_TEXT_PT, MAX_TEXT_PT),
                strings,
                cx,
            );
            font_size_texts.extend(
                custom
                    .iter()
                    .map(|_| strings.setting_text_custom.to_owned()),
            );
            font_size_rows.extend(custom);
        }

        font_size_texts.push(text(
            strings.setting_timer_size,
            strings.setting_timer_size_desc,
            timer_size_label(strings, settings.timer_size),
        ));
        font_size_rows.push(self.timer_size_row(settings, strings, cx));
        if settings.timer_size == TimerSize::Custom {
            let custom = self.custom_size_rows(
                strings.setting_timer_custom,
                &self.timer_pt,
                self.timer_pt_invalid,
                (MIN_TIMER_PT, MAX_TIMER_PT),
                strings,
                cx,
            );
            font_size_texts.extend(
                custom
                    .iter()
                    .map(|_| strings.setting_timer_custom.to_owned()),
            );
            font_size_rows.extend(custom);
        }

        for event in WcaEvent::ALL {
            let rows = self.event_size_rows(EventSizeKind::Timer, event, settings, strings, cx);
            let label = strings
                .setting_timer_size_event
                .replace("{event}", event.name());
            font_size_texts.extend(rows.iter().map(|_| label.clone()));
            font_size_rows.extend(rows);
        }

        font_size_texts.push(text(
            strings.setting_scramble_size,
            strings.setting_scramble_size_desc,
            scramble_size_label(strings, settings.scramble_size),
        ));
        font_size_rows.push(self.scramble_size_row(settings, strings, cx));
        if settings.scramble_size == ScrambleSize::Custom {
            let custom = self.custom_size_rows(
                strings.setting_scramble_custom,
                &self.scramble_pt,
                self.scramble_pt_invalid,
                (MIN_SCRAMBLE_PT, MAX_SCRAMBLE_PT),
                strings,
                cx,
            );
            font_size_texts.extend(
                custom
                    .iter()
                    .map(|_| strings.setting_scramble_custom.to_owned()),
            );
            font_size_rows.extend(custom);
        }
        for event in WcaEvent::ALL {
            let rows = self.event_size_rows(EventSizeKind::Scramble, event, settings, strings, cx);
            let label = strings
                .setting_scramble_size_event
                .replace("{event}", event.name());
            font_size_texts.extend(rows.iter().map(|_| label.clone()));
            font_size_rows.extend(rows);
        }
        let font_size = tagged(font_size_texts, font_size_rows);

        let solve_list = tagged(
            [
                state(
                    strings.setting_solve_list_ao5,
                    strings.setting_solve_list_ao5_desc,
                    settings.solve_list_ao5,
                ),
                state(
                    strings.setting_always_show_scrollbar,
                    strings.setting_always_show_scrollbar_desc,
                    settings.always_show_scrollbar,
                ),
            ],
            [
                self.switch(
                    "solve-list-ao5",
                    strings.setting_solve_list_ao5,
                    strings.setting_solve_list_ao5_desc,
                    settings.solve_list_ao5,
                    |settings, value| settings.solve_list_ao5 = value,
                    cx,
                ),
                self.switch(
                    "always-show-scrollbar",
                    strings.setting_always_show_scrollbar,
                    strings.setting_always_show_scrollbar_desc,
                    settings.always_show_scrollbar,
                    |settings, value| settings.always_show_scrollbar = value,
                    cx,
                ),
            ],
        );

        let shortcuts = tagged(
            [text(
                strings.settings_keybindings,
                strings.setting_shortcuts_desc,
                strings.settings_open_shortcuts,
            )],
            [row(
                strings.settings_keybindings,
                strings.setting_shortcuts_desc,
                Button::new("open-shortcuts")
                    .small()
                    .outline()
                    .label(strings.settings_open_shortcuts)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.app.update(cx, |app, cx| app.open_help(window, cx));
                    })),
                cx,
            )],
        );

        let transfer = tagged(
            [
                text(
                    strings.setting_import,
                    strings.setting_import_desc,
                    strings.import_button,
                ),
                text(
                    strings.setting_export,
                    strings.setting_export_desc,
                    strings.export_button,
                ),
            ],
            Self::transfer_rows(strings, cx),
        );
        let backups = tagged(
            [
                text(
                    strings.setting_backup_folder,
                    "",
                    strings.backup_folder_button,
                ),
                text(
                    strings.setting_backup_now,
                    strings.setting_backup_now_desc,
                    strings.backup_now_button,
                ),
            ],
            self.backup_rows(strings, cx),
        );

        let mut update_texts = vec![
            state(
                strings.setting_check_updates,
                strings.setting_check_updates_desc,
                settings.updates.auto_check,
            ),
            format!(
                "{} {} {} {}",
                strings.setting_update_channel,
                strings.setting_update_channel_desc,
                strings.update_channel_stable,
                strings.update_channel_prerelease
            ),
            format!(
                "{} {} {} {} {}",
                strings.setting_update_frequency,
                strings.setting_update_frequency_desc,
                strings.update_frequency_start,
                strings.update_frequency_daily,
                strings.update_frequency_weekly
            ),
            text(
                strings.setting_check_now,
                strings.setting_check_now_desc,
                "",
            ),
        ];
        if let Some(version) = &settings.updates.skipped_version {
            update_texts.push(format!(
                "{} {}",
                strings.setting_skipping.replace("{version}", version),
                strings.setting_clear_skip
            ));
        }
        let updates = tagged(update_texts, self.update_rows(strings, settings, cx));

        [
            (Category::General, strings.setting_language, language),
            (
                Category::General,
                strings.settings_notifications,
                notifications,
            ),
            (Category::Timer, strings.settings_timer, timer),
            (Category::Timer, strings.settings_scrambles, scrambles),
            (Category::Appearance, strings.settings_theme, themes),
            (Category::Appearance, strings.settings_font_size, font_size),
            (
                Category::Appearance,
                strings.settings_solve_list,
                solve_list,
            ),
            (Category::Shortcuts, strings.settings_keybindings, shortcuts),
            (Category::Data, strings.settings_import_export, transfer),
            (Category::Data, strings.settings_backups, backups),
            (Category::Updates, strings.settings_updates, updates),
        ]
        .into_iter()
        .map(|(category, heading, rows)| Section {
            category,
            heading,
            rows,
        })
        .collect()
    }
}

/// A number field and, while its text is wrong, the message saying why.
/// `description` shows under the label, with `{min}` and `{max}` filled in.
fn number_rows(
    label: impl Into<SharedString>,
    description: &'static str,
    input: &Entity<InputState>,
    invalid: bool,
    (min, max): (u32, u32),
    strings: &'static Strings,
    cx: &App,
) -> Vec<AnyElement> {
    let range = description
        .replace("{min}", &min.to_string())
        .replace("{max}", &max.to_string());
    let mut rows = vec![row(
        label,
        range,
        div().w(Rems(6.)).child(Input::new(input).small().w_full()),
        cx,
    )];
    if invalid {
        let message = strings
            .size_custom_invalid
            .replace("{min}", &min.to_string())
            .replace("{max}", &max.to_string());
        rows.push(
            div()
                .px_4()
                .py_2()
                .text_sm()
                .text_color(gpui_kit::red())
                .child(message)
                .into_any_element(),
        );
    }
    rows
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

/// The custom size fields of every event, filled from the settings.
fn new_event_pts(
    settings: &Settings,
    window: &mut Window,
    cx: &mut App,
) -> BTreeMap<(EventSizeKind, WcaEvent), Entity<InputState>> {
    let mut pts = BTreeMap::new();
    for event in WcaEvent::ALL {
        for (kind, custom) in [
            (
                EventSizeKind::Scramble,
                settings.scramble_choice(event).custom_pt,
            ),
            (EventSizeKind::Timer, settings.timer_choice(event).custom_pt),
        ] {
            pts.insert(
                (kind, event),
                cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(custom.map_or_default(|pt| pt.to_string()))
                        .placeholder("pt")
                }),
            );
        }
    }
    pts
}

/// Wires an event's size field to its settings, flagging invalid input.
fn observe_event_size_input(
    input: &Entity<InputState>,
    window: &mut Window,
    cx: &mut Context<SettingsView>,
    (kind, event): (EventSizeKind, WcaEvent),
) {
    cx.subscribe_in(
        input,
        window,
        move |view, input, event_: &InputEvent, window, cx| {
            if !matches!(event_, InputEvent::Change) {
                return;
            }
            let text = input.read(cx).value().to_string();
            let parsed = match kind {
                EventSizeKind::Scramble => parse_scramble_pt(&text),
                EventSizeKind::Timer => parse_timer_pt(&text),
            };
            match parsed {
                Ok(points) => {
                    view.event_pt_invalid.remove(&(kind, event));
                    view.change(window, cx, move |settings| match kind {
                        EventSizeKind::Scramble => settings.set_scramble_custom_pt(event, points),
                        EventSizeKind::Timer => settings.set_timer_custom_pt(event, points),
                    });
                }
                Err(InvalidSizePt) => {
                    view.event_pt_invalid.insert((kind, event));
                }
            }
            cx.notify();
        },
    )
    .detach();
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Theme files can be added, renamed or deleted while the dialog is
        // open, so read them again on every render.
        self.themes = persistence::list_themes().unwrap_or_default();
        let app = self.app.read(cx);
        let strings = app.strings();
        let settings = app.settings.clone();

        let search = query(&self.search, cx);
        let searching = !search.is_empty();
        // A search looks through every category; a row matches on its own text,
        // its section and its category.
        let sections: Vec<Section> = self
            .sections(&settings, cx)
            .into_iter()
            .filter(|section| searching || section.category == self.category)
            .filter_map(|mut section| {
                let (heading, category) =
                    (section.heading, category_label(strings, section.category));
                section.rows.retain(|(text, _)| {
                    cube_core::search::matches(&search, &[text.as_str(), heading, category])
                });
                (!section.rows.is_empty()).then_some(section)
            })
            .collect();
        let listed: Vec<SettingsCategory> = if searching {
            SettingsCategory::ALL
                .into_iter()
                .filter(|category| sections.iter().any(|section| section.category == *category))
                .collect()
        } else {
            SettingsCategory::ALL.to_vec()
        };
        let no_results = sections.is_empty();

        let mut first_row = 0;
        let mut cards = Vec::with_capacity(sections.len());
        for section in sections {
            let rows: Vec<AnyElement> = section.rows.into_iter().map(|(_, row)| row).collect();
            let count = rows.len();
            cards.push(self.section(section.heading, rows, first_row, cx));
            first_row += count;
        }

        let width = dialog_width(DialogSize::SETTINGS, window);
        let nav_width = px(settings_nav_width(f32::from(width), text_scale(window)));
        let theme = cx.theme();
        let (background, border, muted) = (theme.background, theme.border, theme.muted_foreground);
        let (sidebar, radius) = (theme.sidebar, theme.radius_lg);
        let location = if searching {
            strings.settings_search_results
        } else {
            category_label(strings, self.category)
        };

        h_flex()
            .h(dialog_body_height(DialogSize::SETTINGS, window))
            .w_full()
            .items_start()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                if focus_on_slash(event, &view.search, window, cx) {
                    cx.stop_propagation();
                }
            }))
            .child(
                v_flex()
                    .h_full()
                    .w(nav_width)
                    .flex_none()
                    .bg(sidebar)
                    .border_r_1()
                    .border_color(border)
                    .rounded_l(radius)
                    .child(search_bar(&self.search, cx))
                    .child(self.render_categories(&listed, searching, strings, window, cx)),
            )
            .child(
                v_flex()
                    .h_full()
                    .flex_1()
                    .min_w_0()
                    .child(pane_header(location, strings, cx))
                    .child(
                        FadeScroll::new("settings-scroll", background)
                            .track(&self.pane_scroll)
                            .flex_1()
                            .min_h_0()
                            .px_6()
                            .py_5()
                            .gap_6()
                            .on_key_down(cx.listener(Self::on_pane_key))
                            .children(cards)
                            .when(no_results, |column| {
                                column.child(
                                    div()
                                        .text_sm()
                                        .text_color(muted)
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

/// The `Settings / <Category>` breadcrumb and the close button.
fn pane_header(location: &'static str, strings: &'static Strings, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    h_flex()
        .h(HEADER_HEIGHT)
        .flex_none()
        .px_4()
        .gap_2()
        .justify_between()
        .border_b_1()
        .border_color(theme.border)
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .gap_2()
                .text_sm()
                .child(div().text_color(muted).child(strings.settings))
                .child(div().text_color(muted).child("/"))
                .child(div().truncate().font_medium().child(location)),
        )
        // Last in the tab order, so Tab from the categories lands on the first
        // setting.
        .child(dialog_close_button(strings.close).tab_index(1))
}

/// A category's name in the list and the breadcrumb.
fn category_label(strings: &'static Strings, category: SettingsCategory) -> &'static str {
    match category {
        SettingsCategory::General => strings.settings_general,
        SettingsCategory::Timer => strings.settings_timer,
        SettingsCategory::Appearance => strings.settings_appearance,
        SettingsCategory::Shortcuts => strings.settings_keybindings,
        SettingsCategory::Data => strings.settings_data,
        SettingsCategory::Updates => strings.settings_updates,
    }
}

/// A category's icon in the list.
fn category_icon(category: SettingsCategory) -> IconName {
    match category {
        SettingsCategory::General => IconName::Settings,
        SettingsCategory::Timer => IconName::Timer,
        SettingsCategory::Appearance => IconName::Palette,
        SettingsCategory::Shortcuts => IconName::Keyboard,
        SettingsCategory::Data => IconName::Database,
        SettingsCategory::Updates => IconName::RefreshCw,
    }
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

/// The menu label for a preview size.
fn preview_size_label(strings: &'static Strings, size: PreviewSize) -> &'static str {
    match size {
        PreviewSize::Small => strings.preview_size_small,
        PreviewSize::Normal => strings.preview_size_normal,
        PreviewSize::Big => strings.preview_size_big,
        PreviewSize::Huge => strings.preview_size_huge,
    }
}

/// The title of an event's preview size row.
fn preview_size_title(strings: &'static Strings, event: WcaEvent) -> String {
    strings
        .setting_preview_size
        .replace("{event}", event.name())
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

/// A label over a short description, with the control aligned to the right.
fn row(
    label: impl Into<SharedString>,
    description: impl Into<SharedString>,
    control: impl IntoElement,
    cx: &App,
) -> AnyElement {
    let description = description.into();
    h_flex()
        .min_h(ROW_MIN_HEIGHT)
        .px_4()
        .py_2p5()
        .gap_6()
        .justify_between()
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(div().text_sm().font_medium().child(label.into()))
                .when(!description.is_empty(), |text| {
                    text.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(description),
                    )
                }),
        )
        .child(div().flex_none().child(control))
        .into_any_element()
}

impl CubeApp {
    #[allow(clippy::too_many_lines, reason = "one field per size setting")]
    pub(crate) fn open_settings(&self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity();
        // cube-tui creates this file too, so both apps start with a theme to
        // pick from.
        let _ = persistence::ensure_default_theme();
        let themes = persistence::list_themes().unwrap_or_default();
        let scramble_custom = self.settings.scramble_custom_pt;
        let text_custom = self.settings.text_custom_pt;
        let timer_custom = self.settings.timer_custom_pt;
        let min_solves = self.settings.best_min_solves;
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
        let event_pts = new_event_pts(&self.settings, window, cx);
        let best_min_solves = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(min_solves.to_string())
                .placeholder(DEFAULT_BEST_MIN_SOLVES.to_string())
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
            for (&key, input) in &event_pts {
                observe_event_size_input(input, window, cx, key);
            }
            observe_size_input(
                &best_min_solves,
                window,
                cx,
                parse_best_min_solves,
                |settings, count| {
                    settings.best_min_solves = count.unwrap_or(DEFAULT_BEST_MIN_SOLVES);
                },
                |view, invalid| view.best_min_solves_invalid = invalid,
            );
            SettingsView {
                app: app.clone(),
                event_pts: event_pts.clone(),
                event_pt_invalid: BTreeSet::new(),
                themes,
                scramble_pt: scramble_pt.clone(),
                scramble_pt_invalid: false,
                text_pt: text_pt.clone(),
                text_pt_invalid: false,
                timer_pt: timer_pt.clone(),
                timer_pt_invalid: false,
                best_min_solves: best_min_solves.clone(),
                best_min_solves_invalid: false,
                search,
                focus: cx.focus_handle(),
                category: SettingsCategory::default(),
                categories_focus: cx.focus_handle().tab_stop(true),
                pane_scroll: ScrollHandle::default(),
                row_focus: Vec::new(),
                revealed_row: Rc::default(),
            }
        });
        let first_build = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            let view = view.clone();
            if first_build.replace(false) {
                let focus = view.read(cx).categories_focus.clone();
                focus.focus(window, cx);
            }
            dialog
                .close_button(false)
                .p_0()
                .w(dialog_width(DialogSize::SETTINGS, window))
                .close_on_enter()
                .content(move |content, _, _| content.px_0().child(view.clone()))
        });
    }
}
