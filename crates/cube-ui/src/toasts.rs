//! Timeouts for the short notifications that report an action's result.
//!
//! gpui-component pauses every autohide countdown while any toast stack is
//! hovered or focused, so its own clock cannot guarantee that a notice leaves.
//! cube owns one deadline per toast instead and closes it from the app's
//! regular tick; the notification layer still draws the enter and exit
//! animations. Toasts with an action to reach (Undo, an update to install, or
//! a startup load error) are pushed directly and are never scheduled here.

use std::time::Instant;

use cube_core::toast::ToastSchedule;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::notification::Notification;
use gpui_kit::{App, Global, Window};

/// Identifies the toasts cube is allowed to close on a deadline.
struct TransientToast;

/// The outstanding toast deadlines, shared by every push site in the window.
#[derive(Default)]
pub struct ToastState(ToastSchedule);

impl Global for ToastState {}

/// Pushes a notification that dismisses itself after a few seconds.
///
/// The library's autohide is turned off so a paused stack cannot keep this
/// toast (and the ones behind it) on screen. Clicking the toast or its close
/// button still dismisses it immediately.
pub fn push(notification: Notification, window: &mut Window, cx: &mut App) {
    let id = cx.default_global::<ToastState>().0.push(Instant::now());
    window.push_notification(
        notification
            .id1::<TransientToast>(id)
            .autohide(false)
            // The notification dismisses itself before calling this handler;
            // having a handler is what makes a click on the body dismiss it.
            .on_click(|_, _, _| {})
            .on_close(move |_, cx| cx.global_mut::<ToastState>().0.cancel(id)),
        cx,
    );
}

/// Closes every toast whose deadline has passed. One clock serves all of them,
/// and a toast pushed later gets a deadline of its own.
pub fn expire(window: &mut Window, cx: &mut App) {
    let now = Instant::now();
    if cx
        .try_global::<ToastState>()
        .is_none_or(|state| state.0.is_empty())
    {
        return;
    }
    let expired = cx.global_mut::<ToastState>().0.expired(now);
    for id in expired {
        window.remove_notification1::<TransientToast>(id, cx);
    }
}
