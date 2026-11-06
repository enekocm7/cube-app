//! Tagging solves: the editor shared by the solve details and the tag dialog,
//! the tags of the next solves, and renaming and deleting a tag everywhere.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cube_core::dialogs::DialogSize;
use cube_core::tags::{self, MAX_TAG_CHARS};
use cube_core::{TagChange, TagError};
use cube_locale::{Language, Strings};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{
    ActiveTheme as _, Selectable as _, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::CubeApp;
use crate::details::{field_error, field_label, keep_focus};
use crate::dialog_size::{dialog_body, dialog_body_height, dialog_title, dialog_width};
use crate::dialog_submit::{DialogSubmit as _, Submit, close_action, on_press};
use crate::palette::Palette;
use crate::scroll_fade::FadeScroll;

/// Height of a row of the tag list.
const ROW_HEIGHT: Rems = Rems(2.5);
/// Width of the tag list's count of solves.
const COUNT_WIDTH: Rems = Rems(4.);

/// Says why a tag cannot be used.
fn error_text(error: TagError, strings: &Strings) -> String {
    match error {
        TagError::Invalid => strings
            .tag_invalid
            .replace("{max}", &MAX_TAG_CHARS.to_string()),
        TagError::Exists => strings.tag_exists.to_owned(),
    }
}

/// Picks tags from those known and takes new ones. It changes nothing until
/// the dialog around it saves.
pub(crate) struct TagEditor {
    known: Vec<String>,
    selected: Vec<String>,
    input: Entity<InputState>,
    error: Option<String>,
    strings: &'static Strings,
}

impl TagEditor {
    pub(crate) fn new(
        selected: Vec<String>,
        known: Vec<String>,
        strings: &'static Strings,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let input =
            cx.new(|cx| InputState::new(window, cx).placeholder(strings.tag_new_placeholder));
        cx.new(|_| Self {
            known,
            selected,
            input,
            error: None,
            strings,
        })
    }

    /// The tags chosen so far.
    pub(crate) fn selected(&self) -> Vec<String> {
        self.selected.clone()
    }

    /// Takes in the tag being typed. Returns whether the dialog may go on to
    /// save: not when the text is no tag, and not when Enter in the field was
    /// only adding one, so a second Enter is what saves.
    pub(crate) fn ready_to_save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let typing = self.input.focus_handle(cx).is_focused(window);
        let text = self.input.read(cx).value().to_string();
        if text.trim().is_empty() {
            return true;
        }
        self.add(&text, window, cx) && !typing
    }

    /// Adds a typed tag, clearing the field. Returns whether it was valid.
    fn add(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.error = None;
        let added = match tags::parse(text) {
            Ok(tag) => {
                let tag = tags::spelled(&self.known, tag);
                if !tags::has(&self.known, &tag) {
                    self.known.push(tag.clone());
                    self.known.sort_by_cached_key(|tag| tag.to_lowercase());
                }
                if !tags::has(&self.selected, &tag) {
                    self.selected.push(tag);
                }
                self.input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                true
            }
            Err(error) => {
                self.error = Some(error_text(error, self.strings));
                false
            }
        };
        cx.notify();
        added
    }
}

impl Render for TagEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let strings = self.strings;
        let muted = cx.theme().muted_foreground;
        let red = Palette::of(cx).red;
        let choices: Vec<_> = self
            .known
            .iter()
            .enumerate()
            .map(|(index, tag)| {
                let name = tag.clone();
                let chosen = tags::has(&self.selected, tag);
                Button::new(("tag-choice", index))
                    .outline()
                    .small()
                    .when(chosen, |button| button.icon(IconName::Check))
                    .label(tag.clone())
                    .selected(chosen)
                    .on_click(cx.listener(move |editor, _, _, cx| {
                        tags::toggle(&mut editor.selected, &name);
                        cx.notify();
                    }))
            })
            .collect();
        v_flex()
            .gap_1()
            .child(field_label(strings.field_tags, muted))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(keep_focus("field-tag", Input::new(&self.input))),
                    )
                    .child(
                        Button::new("tag-add")
                            .outline()
                            .label(strings.tag_add)
                            .on_click(cx.listener(|editor, _, window, cx| {
                                let text = editor.input.read(cx).value().to_string();
                                editor.add(&text, window, cx);
                            })),
                    ),
            )
            .children(field_error(self.error.as_ref(), red))
            .when(!choices.is_empty(), |column| {
                column.child(h_flex().pt_1().flex_wrap().gap_1().children(choices))
            })
    }
}

impl CubeApp {
    /// Says so and returns `false` when tags cannot be saved: the file on disk
    /// could not be read, and nothing is written over it.
    fn tags_usable(&self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.tags_available {
            let message = self.strings().tags_unavailable;
            window.push_notification(Notification::warning(message), cx);
        }
        self.tags_available
    }

    /// Picks tags in a dialog of their own and hands them to `apply`.
    fn open_tag_picker(
        &self,
        title: &'static str,
        selected: Vec<String>,
        apply: impl Fn(&mut Self, Vec<String>, &mut Window, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let strings = self.strings();
        let editor = TagEditor::new(selected, self.known_tags(), strings, window, cx);
        let app = cx.entity().downgrade();
        let apply = Rc::new(apply);
        let first_build = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first_build.replace(false) {
                let input = editor.read(cx).input.clone();
                input.focus_handle(cx).focus(window, cx);
            }
            let (editor_for_save, app, apply) = (editor.clone(), app.clone(), Rc::clone(&apply));
            let save: Submit = Rc::new(move |window, cx| {
                if !editor_for_save.update(cx, |editor, cx| editor.ready_to_save(window, cx)) {
                    return;
                }
                let tags = editor_for_save.read(cx).selected();
                app.update(cx, |app, cx| {
                    apply(app, tags, window, cx);
                    app.focus.focus(window, cx);
                })
                .ok();
                window.close_dialog(cx);
            });
            let background = cx.theme().background;
            dialog
                .title(dialog_title(title, strings.close))
                .close_button(false)
                .w(dialog_width(DialogSize::COMPACT, window))
                .submit_on_enter(&save)
                .child(dialog_body(
                    FadeScroll::new("tag-picker", background)
                        .max_h(dialog_body_height(DialogSize::COMPACT, window))
                        .child(editor.clone()),
                ))
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("tag-cancel")
                                .outline()
                                .label(strings.cancel)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("tag-save")
                                .primary()
                                .label(strings.save)
                                .on_click(on_press(&save)),
                        ),
                )
        });
    }

    /// Picks the tags of the solve at `index`.
    pub(crate) fn open_solve_tags(
        &self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(solve) = self.session().solves().get(index) else {
            return;
        };
        let selected = solve.tags().to_vec();
        if self.tags_usable(window, cx) {
            let title = self.strings().tags_solve_title;
            let apply = move |app: &mut Self, tags, window: &mut Window, cx: &mut Context<Self>| {
                app.set_solve_tags(index, tags, window, cx);
            };
            self.open_tag_picker(title, selected, apply, window, cx);
        }
    }

    /// Picks the tags the next solves get.
    pub(crate) fn open_active_tags(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tags_usable(window, cx) {
            let title = self.strings().tags_active_title;
            let selected = self.settings.active_tags.clone();
            self.open_tag_picker(title, selected, Self::set_active_tags, window, cx);
        }
    }

    /// Lists every tag, to rename or delete it.
    ///
    /// The dialog reads the tags on every build, so a change made from here
    /// shows at once.
    pub(crate) fn open_manage_tags(&self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.tags_usable(window, cx) {
            return;
        }
        let app = cx.entity();
        window.open_dialog(cx, move |dialog, window, cx| {
            let state = app.read(cx);
            let (strings, language) = (state.strings(), state.language());
            let rows = render_tag_rows(&app, strings, language, cx);
            let empty = rows.is_empty();
            let theme = cx.theme();
            let (background, muted) = (theme.background, theme.muted_foreground);
            dialog
                .title(dialog_title(strings.tags_manage, strings.close))
                .close_button(false)
                .w(dialog_width(DialogSize::COMPACT, window))
                .close_on_enter()
                .child(dialog_body(
                    FadeScroll::new("tag-list", background)
                        .max_h(dialog_body_height(DialogSize::COMPACT, window))
                        .when(empty, |list| {
                            list.child(div().text_color(muted).child(strings.tags_none))
                        })
                        .children(rows),
                ))
                .footer(
                    h_flex().w_full().justify_end().child(
                        Button::new("tags-close")
                            .primary()
                            .label(strings.close)
                            .on_click(on_press(&close_action())),
                    ),
                )
        });
    }

    /// Asks for a new name for `from`.
    fn open_rename_tag(&self, from: String, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.strings();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(from.clone()));
        let app = cx.entity().downgrade();
        let error = Rc::new(RefCell::new(None));
        let first_build = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first_build.replace(false) {
                input.focus_handle(cx).focus(window, cx);
            }
            let (input_for_save, app, from) = (input.clone(), app.clone(), from.clone());
            let error_on_save = Rc::clone(&error);
            let save: Submit = Rc::new(move |window, cx| {
                let text = input_for_save.read(cx).value().to_string();
                let known = app
                    .read_with(cx, |app, _| app.known_tags())
                    .unwrap_or_default();
                match tags::check_rename(&known, &from, &text) {
                    Ok(to) => {
                        let from = from.clone();
                        app.update(cx, |app, cx| {
                            app.apply_tag_change(TagChange::Rename { from, to }, window, cx);
                        })
                        .ok();
                        window.close_dialog(cx);
                    }
                    Err(problem) => {
                        *error_on_save.borrow_mut() = Some(error_text(problem, strings));
                        window.refresh();
                    }
                }
            });
            let red = Palette::of(cx).red;
            dialog
                .title(dialog_title(strings.tag_rename, strings.close))
                .close_button(false)
                .w(dialog_width(DialogSize::COMPACT, window))
                .submit_on_enter(&save)
                .child(
                    v_flex()
                        .py_2()
                        .gap_1()
                        .child(keep_focus("tag-rename-field", Input::new(&input)))
                        .children(field_error(error.borrow().as_ref(), red)),
                )
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("tag-rename-cancel")
                                .outline()
                                .label(strings.cancel)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("tag-rename-save")
                                .primary()
                                .label(strings.save)
                                .on_click(on_press(&save)),
                        ),
                )
        });
    }

    /// Deletes `tag`, asking first when solves use it.
    fn confirm_delete_tag(
        &mut self,
        tag: String,
        solves: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if solves == 0 {
            self.apply_tag_change(TagChange::Remove { tag }, window, cx);
            return;
        }
        let strings = self.strings();
        let message = strings
            .tag_delete_confirm
            .replace("{tag}", &tag)
            .replace("{count}", &self.language().format_count(solves as u64));
        let on_confirm = cx.listener(move |app: &mut Self, (): &(), window, cx| {
            let tag = tag.clone();
            app.apply_tag_change(TagChange::Remove { tag }, window, cx);
        });
        self.open_confirm(strings.tag_delete, message, on_confirm, window, cx);
    }

    /// The line on the timer screen that shows and opens the next solves' tags.
    pub(crate) fn render_active_tags(&self, cx: &mut Context<Self>) -> Option<Div> {
        if !self.tags_available {
            return None;
        }
        let strings = self.strings();
        let active = &self.settings.active_tags;
        let label = if active.is_empty() {
            strings.tags_active_none.to_owned()
        } else {
            strings
                .tags_active_label
                .replace("{tags}", &active.join(", "))
        };
        Some(
            div().flex_none().max_w_full().pt_2().child(
                Button::new("active-tags")
                    .ghost()
                    .xsmall()
                    .tab_stop(false)
                    .child(div().min_w_0().truncate().child(label))
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.open_active_tags(window, cx);
                    })),
            ),
        )
    }
}

/// One row per tag: its name, how many solves use it and its buttons.
fn render_tag_rows(
    app: &Entity<CubeApp>,
    strings: &'static Strings,
    language: Language,
    cx: &App,
) -> Vec<AnyElement> {
    let muted = cx.theme().muted_foreground;
    app.read(cx)
        .tag_usage
        .iter()
        .enumerate()
        .map(|(index, usage)| {
            let (rename_app, delete_app) = (app.clone(), app.clone());
            let (rename_tag, delete_tag) = (usage.tag.clone(), usage.tag.clone());
            let solves = usage.solves;
            h_flex()
                .h(ROW_HEIGHT)
                .gap_2()
                .child(div().flex_1().min_w_0().truncate().child(usage.tag.clone()))
                .child(
                    div()
                        .w(COUNT_WIDTH)
                        .flex_none()
                        .truncate()
                        .text_right()
                        .text_sm()
                        .text_color(muted)
                        .child(language.format_count(solves as u64)),
                )
                .child(
                    Button::new(("tag-rename", index))
                        .ghost()
                        .small()
                        .icon(IconName::Pencil)
                        .tooltip(strings.tag_rename)
                        .accessibility_label(strings.tag_rename)
                        .on_click(move |_, window, cx| {
                            let tag = rename_tag.clone();
                            rename_app.update(cx, |app, cx| {
                                app.open_rename_tag(tag, window, cx);
                            });
                        }),
                )
                .child(
                    Button::new(("tag-delete", index))
                        .ghost()
                        .small()
                        .icon(IconName::Trash)
                        .tooltip(strings.tag_delete)
                        .accessibility_label(strings.tag_delete)
                        .on_click(move |_, window, cx| {
                            let tag = delete_tag.clone();
                            delete_app.update(cx, |app, cx| {
                                app.confirm_delete_tag(tag, solves, window, cx);
                            });
                        }),
                )
                .into_any_element()
        })
        .collect()
}
