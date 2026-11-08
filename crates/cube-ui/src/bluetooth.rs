//! The Bluetooth dialog, and following a connected timer.

use std::cell::Cell;
use std::rc::Rc;

use cube_bluetooth::{
    Client, ConnectError, Device, Link, LinkEvent, Picker, Problem, ScanError, ScanEvent,
};
use cube_core::{HardwareState, TimerPhase};
use cube_locale::Strings;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, Selectable as _, Sizable as _, WindowExt as _,
    h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CubeApp, Screen};
use crate::dialog_size::{DialogSize, dialog_body, dialog_body_height, dialog_title, dialog_width};
use crate::dialog_submit::{DialogSubmit as _, Submit, close_action, on_press};
use crate::palette::Palette;
use crate::scroll_fade::FadeScroll;

const DIALOG_SIZE: DialogSize = DialogSize::COMPACT;
const ROW_HEIGHT: Rems = Rems(2.25);
const ICON_SIZE: Pixels = px(16.);
/// Room for the status line above the device list.
const STATUS_HEIGHT: Rems = Rems(2.);
/// How far the title bar's button leans towards its neighbour. Its symbol is
/// much narrower than the other icons there, so at the same spacing it looks
/// set apart from them. This stays inside the gap, so the buttons never overlap.
const TITLE_BAR_NUDGE: Rems = Rems(-0.15);

/// How far along the connection to a timer is.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkStatus {
    None,
    Connecting,
    Connected,
}

/// The connection to a timer, made or being made.
pub(crate) struct TimerLink {
    device: Device,
    connected: bool,
    link: Link,
    /// Delivers the connection's reports. Dropping it stops listening.
    _events: Task<()>,
}

impl CubeApp {
    /// Returns how far along the connection to a timer is.
    pub(crate) fn link_status(&self) -> LinkStatus {
        match &self.timer_link {
            None => LinkStatus::None,
            Some(link) if link.connected => LinkStatus::Connected,
            Some(_) => LinkStatus::Connecting,
        }
    }

    /// Returns whether a Bluetooth timer does the timing, or is about to, so
    /// the timer key and the digits must leave it alone.
    pub(crate) fn hardware_timing(&self) -> bool {
        self.timer_link.is_some()
    }

    /// Returns whether the screen is ready for a hardware timer to time its
    /// attempt. Multi-Blind and relays need their scrambles first; every other
    /// screen times a normal solve.
    fn hardware_attempt_ready(&self) -> bool {
        match self.screen {
            Screen::MultiBlind => self.multi_blind_ready(),
            Screen::Relays => self.relays_ready(),
            _ => true,
        }
    }

    /// The title bar's Bluetooth button, whose icon shows the connection.
    pub(crate) fn render_bluetooth_button(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let title = self.strings().bluetooth_title;
        let status = self.link_status();
        Button::new("open-bluetooth")
            .ghost()
            .small()
            .mr(TITLE_BAR_NUDGE)
            .icon(match status {
                LinkStatus::None => IconName::Bluetooth,
                LinkStatus::Connecting => IconName::BluetoothSearching,
                LinkStatus::Connected => IconName::BluetoothConnected,
            })
            .selected(status == LinkStatus::Connected)
            .tooltip(title)
            .accessibility_label(title)
            .tab_stop(false)
            .on_click(cx.listener(|app, _, window, cx| {
                if app.timer.is_active() {
                    app.focus.focus(window, cx);
                } else {
                    app.open_bluetooth(window, cx);
                }
            }))
    }

    /// Opens the dialog to find, connect or disconnect a timer.
    pub(crate) fn open_bluetooth(&self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity();
        let strings = self.strings();
        let status = self.link_status();
        let client = self.bluetooth.clone();
        let view = cx.new(|cx| BluetoothView::new(app, client, status, window, cx));

        let first_build = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first_build.replace(false) {
                // So the arrow keys reach the list before any click.
                let focus = view.read(cx).focus.clone();
                focus.focus(window, cx);
            }
            let submit: Submit = {
                let view = view.clone();
                Rc::new(move |window, cx| {
                    view.update(cx, |view, cx| view.submit(window, cx));
                })
            };
            let (label, enabled) = view.read(cx).primary_action(strings, cx);
            dialog
                .title(dialog_title(strings.bluetooth_title, strings.close))
                .close_button(false)
                .w(dialog_width(DIALOG_SIZE, window))
                .submit_on_enter(&submit)
                .child(view.clone())
                .footer(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .child(
                            Button::new("bluetooth-close")
                                .outline()
                                .label(strings.close)
                                .on_click(on_press(&close_action())),
                        )
                        .child(
                            Button::new("bluetooth-submit")
                                .primary()
                                .label(label)
                                .disabled(!enabled)
                                .on_click(on_press(&submit)),
                        ),
                )
        });
    }

    /// Starts connecting to `device`.
    fn connect_timer(&mut self, device: Device, window: &mut Window, cx: &mut Context<Self>) {
        if self.timer_link.is_some() {
            return;
        }
        // From here the hardware owns attempts, so one in progress from the
        // keyboard could no longer be stopped by it.
        if self.timer.is_active() {
            self.timer.cancel();
            self.toast(self.strings().toast_cancelled, window, cx);
        }
        let (link, events) = self.bluetooth.connect(&device);
        let listener = cx.spawn_in(window, async move |this, cx| {
            while let Some(event) = events.next().await {
                let delivered = this.update_in(cx, |app, window, cx| {
                    app.on_link_event(event, window, cx);
                });
                if delivered.is_err() {
                    break;
                }
            }
        });
        self.timer_link = Some(TimerLink {
            device,
            connected: false,
            link,
            _events: listener,
        });
        cx.notify();
        window.refresh();
    }

    /// Ends the connection, or the attempt to make one. An attempt in progress
    /// goes with it, since nothing would finish it any more.
    fn disconnect_timer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(TimerLink {
            connected, link, ..
        }) = self.timer_link.take()
        else {
            return;
        };
        self.timer.cancel();
        if connected {
            self.toast(self.strings().bluetooth_disconnected, window, cx);
        }
        cx.spawn_in(window, async move |this, cx| {
            if let Err(error) = link.disconnect().await {
                this.update_in(cx, |app, window, cx| {
                    let message = app
                        .strings()
                        .bluetooth_disconnect_failed
                        .replace("{error}", &error);
                    window.push_notification(Notification::error(message), cx);
                })
                .ok();
            }
        })
        .detach();
        cx.notify();
        window.refresh();
    }

    fn on_link_event(&mut self, event: LinkEvent, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.strings();
        match event {
            LinkEvent::Connected => {
                if let Some(link) = &mut self.timer_link {
                    link.connected = true;
                    let message = strings
                        .bluetooth_connected
                        .replace("{name}", &link.device.name);
                    self.toast(&message, window, cx);
                }
            }
            LinkEvent::State(state) => self.follow_timer(state, window, cx),
            LinkEvent::Problem(problem) => self.report_problem(&problem, window, cx),
            LinkEvent::Failed(error) => {
                self.timer_link = None;
                let message = connect_error_message(strings, &error);
                window.push_notification(Notification::error(message), cx);
            }
            LinkEvent::Lost => {
                self.timer_link = None;
                self.timer.cancel();
                // Not a toast: losing the timer matters even with toasts off.
                let toast = Notification::warning(strings.bluetooth_lost.to_owned())
                    .placement(Anchor::BottomRight);
                window.push_notification(toast, cx);
            }
        }
        cx.notify();
        window.refresh();
    }

    /// Mirrors the timer's state, recording the solve when it finishes.
    /// Pressing the timer's reset button reports [`HardwareState::Idle`],
    /// which only puts the digits back to zero, like cube-tui does.
    fn follow_timer(&mut self, state: HardwareState, window: &mut Window, cx: &mut Context<Self>) {
        // Multi-Blind and relays can only be timed here once their scrambles
        // are ready: there would be nothing to record without them, and a
        // normal solve would be recorded over the mode. Leave the timer alone.
        if !self.hardware_attempt_ready() {
            if state == HardwareState::Running {
                self.toast_warning(self.strings().bluetooth_no_attempt, window, cx);
            }
            self.timer.cancel();
            return;
        }
        let running = |phase| matches!(phase, TimerPhase::Running { .. });
        let was_running = running(self.timer.phase());
        if !was_running && state == HardwareState::Running {
            self.prepare_attempt(true);
        }
        let outcome = self.timer.apply_hardware(state, std::time::Instant::now());
        if state == HardwareState::Idle {
            self.last_result = None;
        }
        // Multi-Blind and relays keep their own screen: the hardware times
        // them there, the same as the timer key would.
        if self.timer.is_active()
            && self.screen != Screen::Timer
            && !self.drilling()
            && !matches!(self.screen, Screen::MultiBlind | Screen::Relays)
        {
            self.show_screen(Screen::Timer, cx);
        }
        if let Some(outcome) = outcome {
            self.finish_attempt(outcome, window, cx);
        }
    }

    fn report_problem(&mut self, problem: &Problem, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.strings();
        let lost_solve = match problem {
            Problem::IncompleteUpdate => {
                self.toast_warning(strings.bluetooth_incomplete_update, window, cx);
                return;
            }
            Problem::UnsupportedState(state) => {
                let message = strings
                    .bluetooth_unsupported_state
                    .replace("{state}", &state.to_string());
                self.toast_warning(&message, window, cx);
                return;
            }
            Problem::IncompleteTime => strings.bluetooth_incomplete_time.to_owned(),
            Problem::TimeUnreadable(error) => {
                strings.bluetooth_time_unreadable.replace("{error}", error)
            }
        };
        self.timer.cancel();
        window.push_notification(Notification::error(lost_solve), cx);
    }
}

fn connect_error_message(strings: &Strings, error: &ConnectError) -> String {
    match error {
        ConnectError::NotFound => strings.bluetooth_not_found.to_owned(),
        ConnectError::TimedOut => strings.bluetooth_timed_out.to_owned(),
        ConnectError::Failed(error) => strings.bluetooth_connect_failed.replace("{error}", error),
    }
}

fn scan_error_message(strings: &Strings, error: &ScanError) -> String {
    match error {
        ScanError::NoAdapter => strings.bluetooth_no_adapter.to_owned(),
        ScanError::Failed(error) => strings.bluetooth_scan_failed.replace("{error}", error),
    }
}

/// The dialog's contents. It lives as long as the dialog, and so does its
/// scan: however the dialog closes, dropping this view stops looking.
struct BluetoothView {
    app: Entity<CubeApp>,
    client: Client,
    focus: FocusHandle,
    picker: Picker,
    /// Receives the scan's reports. Dropping it stops the scan.
    scan: Option<Task<()>>,
    /// The connection as last seen, to notice it changing.
    status: LinkStatus,
}

impl BluetoothView {
    fn new(
        app: Entity<CubeApp>,
        client: Client,
        status: LinkStatus,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe_in(&app, window, |view, _, window, cx| view.sync(window, cx))
            .detach();
        let mut view = Self {
            app,
            client,
            focus: cx.focus_handle(),
            picker: Picker::default(),
            scan: None,
            status,
        };
        if status == LinkStatus::None {
            view.start_scan(window, cx);
        }
        view
    }

    /// Follows the connection: scanning only without one, and leaving the
    /// dialog once one is made, as there is nothing more to do here.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let status = self.app.read(cx).link_status();
        let previous = std::mem::replace(&mut self.status, status);
        if status == LinkStatus::None {
            if self.scan.is_none() {
                self.start_scan(window, cx);
                self.focus.focus(window, cx);
            }
        } else {
            self.scan = None;
        }
        if status == previous {
            return;
        }
        if previous == LinkStatus::Connecting && status == LinkStatus::Connected {
            window.close_dialog(cx);
        }
        cx.notify();
    }

    fn start_scan(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.picker.restart();
        let scan = self.client.scan();
        self.scan = Some(cx.spawn_in(window, async move |view, cx| {
            while let Some(event) = scan.next().await {
                let delivered = view.update_in(cx, |view, window, cx| {
                    view.on_scan_event(event, window, cx);
                });
                if delivered.is_err() {
                    break;
                }
            }
        }));
    }

    fn on_scan_event(&mut self, event: ScanEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            ScanEvent::Device(device) => {
                if !self.picker.add(device) {
                    return;
                }
            }
            ScanEvent::Failed(error) => {
                let strings = self.app.read(cx).strings();
                let message = scan_error_message(strings, &error);
                window.push_notification(Notification::error(message), cx);
                self.picker.fail(error);
            }
        }
        cx.notify();
        // The footer's button depends on the list and lives outside this view.
        window.refresh();
    }

    /// The label of the primary button and whether it can be pressed.
    fn primary_action(&self, strings: &'static Strings, cx: &App) -> (&'static str, bool) {
        match self.app.read(cx).link_status() {
            LinkStatus::None => (strings.bluetooth_connect, self.picker.selected().is_some()),
            LinkStatus::Connecting => (strings.cancel, true),
            LinkStatus::Connected => (strings.bluetooth_disconnect, true),
        }
    }

    /// Connects to the selected timer, or ends the current connection.
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.app.read(cx).hardware_timing() {
            self.app
                .update(cx, |app, cx| app.disconnect_timer(window, cx));
        } else if let Some(device) = self.picker.selected().cloned() {
            self.scan = None;
            self.app
                .update(cx, |app, cx| app.connect_timer(device, window, cx));
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "up" => self.picker.select_previous(),
            "down" => self.picker.select_next(),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
        window.refresh();
    }

    /// The line saying what is going on, in the colour that fits it.
    fn status_line(&self, strings: &Strings, cx: &App) -> (String, Hsla) {
        let app = self.app.read(cx);
        let colors = Palette::of(cx);
        let name = app
            .timer_link
            .as_ref()
            .map(|link| link.device.name.as_str());
        match (app.link_status(), name) {
            (LinkStatus::Connected, Some(name)) => (
                strings.bluetooth_connected.replace("{name}", name),
                colors.green,
            ),
            (LinkStatus::Connecting, Some(name)) => (
                strings.bluetooth_connecting.replace("{name}", name),
                colors.yellow,
            ),
            _ => match self.picker.error() {
                Some(error) => (scan_error_message(strings, error), colors.red),
                None if self.picker.devices().is_empty() => (
                    strings.bluetooth_scanning.to_owned(),
                    cx.theme().muted_foreground,
                ),
                None => (
                    strings.bluetooth_pick.to_owned(),
                    cx.theme().muted_foreground,
                ),
            },
        }
    }

    /// One timer, as a button that connects to it.
    fn render_device(
        &self,
        index: usize,
        device: &Device,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        Button::new(("bluetooth-device", index))
            .ghost()
            .w_full()
            .h(ROW_HEIGHT)
            .selected(index == self.picker.selected_index())
            .child(
                h_flex()
                    .w_full()
                    .gap_3()
                    .child(Icon::new(IconName::Bluetooth).size(ICON_SIZE))
                    .child(device.name.clone()),
            )
            .on_click(cx.listener(move |view, _, window, cx| {
                view.picker.select(index);
                view.submit(window, cx);
            }))
    }
}

impl Render for BluetoothView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let strings = self.app.read(cx).strings();
        let (status, status_color) = self.status_line(strings, cx);
        let listing = !self.app.read(cx).hardware_timing();
        let rows: Vec<_> = if listing {
            let devices = self.picker.devices().to_vec();
            devices
                .iter()
                .enumerate()
                .map(|(index, device)| self.render_device(index, device, cx))
                .collect()
        } else {
            Vec::new()
        };
        let max_height =
            dialog_body_height(DIALOG_SIZE, window) - STATUS_HEIGHT.to_pixels(window.rem_size());

        dialog_body(
            v_flex()
                .track_focus(&self.focus)
                .on_key_down(cx.listener(Self::on_key_down))
                .child(
                    div()
                        .h(STATUS_HEIGHT)
                        .text_sm()
                        .text_color(status_color)
                        .child(status),
                )
                .when(!rows.is_empty(), |body| {
                    body.child(
                        FadeScroll::new("bluetooth-devices", cx.theme().background)
                            .max_h(max_height.max(px(0.)))
                            .children(rows),
                    )
                }),
        )
    }
}
