//! The dialog that sizes the scramble preview, the timer and the scramble per
//! event, kept apart from the settings so they do not crowd them.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use cube_core::settings::{
    InvalidSizePt, MAX_SCRAMBLE_PT, MAX_TIMER_PT, MIN_SCRAMBLE_PT, MIN_TIMER_PT, PreviewSize,
    ScrambleSize, TimerSize, parse_scramble_pt, parse_timer_pt,
};
use cube_core::{Settings, WcaEvent};
use cube_locale::Strings;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::CubeApp;
use crate::dialog_size::{DialogSize, dialog_body, dialog_body_height, dialog_title, dialog_width};
use crate::dialog_submit::{DialogSubmit as _, close_action, on_press};
use crate::scroll_fade::FadeScroll;
use crate::settings_view::{choice, number_rows, row, scramble_size_label, timer_size_label};

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

/// The dialog's content: an event picker and that event's three sizes.
struct EventSizesView {
    app: Entity<CubeApp>,
    /// The event whose sizes are shown.
    event: WcaEvent,
    /// The custom size field of each event that can have its own size.
    event_pts: BTreeMap<(EventSizeKind, WcaEvent), Entity<InputState>>,
    /// The event size fields whose text is not a valid size.
    event_pt_invalid: BTreeSet<(EventSizeKind, WcaEvent)>,
}

impl EventSizesView {
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
}

impl Render for EventSizesView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.app.read(cx);
        let strings = app.strings();
        let settings = app.settings.clone();
        let event = self.event;
        let border = cx.theme().border;
        let radius = cx.theme().radius_lg;

        let picker = h_flex()
            .flex_wrap()
            .gap_1p5()
            .children(WcaEvent::ALL.map(|choice_event| {
                choice(
                    SharedString::from(format!("event-size-pick-{choice_event:?}")),
                    choice_event.name(),
                    choice_event == event,
                )
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.event = choice_event;
                    cx.notify();
                }))
            }));

        let mut rows = vec![self.preview_size_row(event, &settings, strings, cx)];
        rows.extend(self.event_size_rows(EventSizeKind::Timer, event, &settings, strings, cx));
        rows.extend(self.event_size_rows(EventSizeKind::Scramble, event, &settings, strings, cx));
        let rows = rows.into_iter().enumerate().map(|(index, row)| {
            div()
                .when(index > 0, |row| row.border_t_1().border_color(border))
                .child(row)
        });

        v_flex().gap_4().child(picker).child(
            v_flex()
                .rounded(radius)
                .border_1()
                .border_color(border)
                .children(rows),
        )
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
    cx: &mut Context<EventSizesView>,
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

impl CubeApp {
    /// Opens the per-event sizes, starting on the event being timed.
    pub(crate) fn open_event_sizes(&self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity();
        let strings = self.strings();
        let event = self.event;
        let event_pts = new_event_pts(&self.settings, window, cx);
        let view = cx.new(|cx| {
            for (&key, input) in &event_pts {
                observe_event_size_input(input, window, cx, key);
            }
            EventSizesView {
                app,
                event,
                event_pts: event_pts.clone(),
                event_pt_invalid: BTreeSet::new(),
            }
        });
        window.open_dialog(cx, move |dialog, window, cx| {
            let background = cx.theme().background;
            dialog
                .title(dialog_title(strings.setting_event_sizes, strings.close))
                .close_button(false)
                .w(dialog_width(DialogSize::FORM, window))
                .close_on_enter()
                .child(dialog_body(
                    FadeScroll::new("event-sizes-scroll", background)
                        .max_h(dialog_body_height(DialogSize::FORM, window))
                        .child(view.clone()),
                ))
                .footer(
                    h_flex().w_full().justify_end().child(
                        Button::new("event-sizes-close")
                            .primary()
                            .label(strings.close)
                            .on_click(on_press(&close_action())),
                    ),
                )
        });
    }
}
