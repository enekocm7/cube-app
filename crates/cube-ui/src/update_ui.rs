//! Update flow: background check on startup, "Update now" with a progress
//! dialog, release notes and "Skip this version".

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use cube_core::UpdateChannel;
use cube_updater::{self as updater, DownloadProgress, InstallKind, Release, UpdateError};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::DialogButtonProps;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::progress::Progress as ProgressBar;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::*;

use crate::app::CubeApp;
use crate::dialog_size::{DialogSize, dialog_body_height, dialog_title, dialog_width};
use crate::scroll_fade::FadeScroll;

/// Current time as a Unix timestamp, for update pacing.
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

/// Live contents of the "Updating cube" dialog.
struct DownloadView {
    version: String,
    progress: Arc<DownloadProgress>,
    strings: &'static cube_locale::Strings,
}

impl Render for DownloadView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let percent = self.progress.percent();
        let detail = percent.map_or_else(|| "…".to_owned(), |percent| format!("{percent:.0}%"));
        let progress = ProgressBar::new("update-progress");
        let progress = match percent {
            Some(percent) => progress.value(percent),
            None => progress.loading(true),
        };
        v_flex()
            .gap_2()
            .child(
                div().text_sm().child(
                    self.strings
                        .update_downloading
                        .replace("{version}", &self.version),
                ),
            )
            .child(progress)
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(detail),
            )
    }
}

impl CubeApp {
    /// Runs once per launch: reports a finished update and kicks off the
    /// automatic check when it is due under the configured frequency.
    pub(crate) fn startup_update_tasks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = updater::CURRENT_VERSION;
        let previous = self
            .settings
            .updates
            .last_run_version
            .replace(current.to_owned());
        if previous.as_deref() != Some(current) {
            self.save_settings(window, cx);
            if previous
                .as_deref()
                .is_some_and(|previous| updater::is_newer(previous, current))
            {
                let message = self
                    .strings()
                    .update_updated_body
                    .replace("{current}", current);
                let title = self.strings().update_updated_title;
                // The notification layer belongs to the root view, which wraps
                // this one and does not exist until construction finishes.
                window.on_next_frame(move |window, cx| {
                    window.push_notification(Notification::success(message).title(title), cx);
                });
            }
        }
        if self.settings.updates.check_due(unix_now()) {
            self.check_for_updates(false, window, cx);
        }
    }

    /// `manual` checks report errors and "you're up to date"; automatic ones
    /// stay silent unless there is something to install.
    pub(crate) fn check_for_updates(
        &mut self,
        manual: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.update_checking || self.updating {
            return;
        }
        self.update_checking = true;
        // Any check (manual or automatic) counts for daily / weekly pacing.
        self.settings.updates.last_check = Some(unix_now());
        self.save_settings(window, cx);
        cx.notify();
        let channel = match self.settings.updates.channel {
            UpdateChannel::Stable => updater::Channel::Stable,
            UpdateChannel::Prerelease => updater::Channel::Prerelease,
        };
        let job = cx
            .background_executor()
            .spawn(async move { updater::fetch_channel(channel) });
        cx.spawn_in(window, async move |this, cx| {
            let result = job.await;
            this.update_in(cx, |this, window, cx| {
                this.update_checking = false;
                this.on_check_result(result, manual, window, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn on_check_result(
        &mut self,
        result: Result<Option<Release>, UpdateError>,
        manual: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let release = match result {
            Ok(release) => release.filter(Release::is_newer_than_current),
            Err(error) => {
                if manual {
                    window.push_notification(
                        Notification::error(
                            self.strings()
                                .update_check_failed_body
                                .replace("{error}", &error.to_string()),
                        )
                        .title(self.strings().update_check_failed_title),
                        cx,
                    );
                }
                return;
            }
        };
        let Some(release) = release else {
            if manual {
                window.push_notification(
                    Notification::success(
                        self.strings()
                            .update_uptodate_body
                            .replace("{version}", updater::CURRENT_VERSION),
                    )
                    .title(self.strings().update_uptodate_title),
                    cx,
                );
            }
            return;
        };
        if release.asset_for(updater::detect_kind()).is_none() {
            if manual {
                window.push_notification(
                    Notification::error(self.strings().update_no_asset)
                        .title(self.strings().update_failed_title),
                    cx,
                );
            }
            return;
        }

        let version = release.version.to_string();
        // A skip only applies to that exact version; a newer release clears it.
        if self
            .settings
            .updates
            .skipped_version
            .as_deref()
            .is_some_and(|skipped| skipped != version)
        {
            self.settings.updates.skipped_version = None;
            self.save_settings(window, cx);
        }
        let skipped = self.settings.updates.skipped_version.as_deref() == Some(version.as_str());
        if skipped && !manual {
            return;
        }
        self.available = Some(Arc::new(release));
        self.notify_available(&version, window, cx);
        cx.notify();
    }

    fn notify_available(&self, version: &str, window: &mut Window, cx: &mut Context<Self>) {
        let entity = cx.entity().downgrade();
        let body = self
            .strings()
            .update_available_body
            .replace("{version}", version);
        let body = body.replace("{current}", updater::CURRENT_VERSION);
        let title = self.strings().update_available_title;
        let update_label = self.strings().update_now;
        let notes_label = self.strings().update_release_notes;
        let skip_label = self.strings().update_skip;
        let notes_entity = entity.clone();
        let skip_entity = entity.clone();
        window.push_notification(
            Notification::info(body)
                .title(title)
                .action(move |_, _, cx| {
                    let entity = entity.clone();
                    Button::new("update-now")
                        .primary()
                        .label(update_label)
                        .on_click(cx.listener(move |note, _, window, cx| {
                            note.dismiss(window, cx);
                            entity
                                .update(cx, |this, cx| this.start_update(window, cx))
                                .ok();
                        }))
                })
                .content(move |_, _, cx| {
                    let notes_entity = notes_entity.clone();
                    let skip_entity = skip_entity.clone();
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("update-notes")
                                .ghost()
                                .small()
                                .label(notes_label)
                                .on_click(cx.listener(move |note, _, window, cx| {
                                    note.dismiss(window, cx);
                                    notes_entity
                                        .update(cx, |this, cx| this.show_release_notes(window, cx))
                                        .ok();
                                })),
                        )
                        .child(
                            Button::new("update-skip")
                                .ghost()
                                .small()
                                .label(skip_label)
                                .on_click(cx.listener(move |note, _, window, cx| {
                                    note.dismiss(window, cx);
                                    skip_entity
                                        .update(cx, |this, cx| this.skip_update(window, cx))
                                        .ok();
                                })),
                        )
                        .into_any_element()
                }),
            cx,
        );
    }

    pub(crate) fn show_release_notes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(release) = self.available.clone() else {
            return;
        };
        let entity = cx.entity().downgrade();
        let strings = self.strings();
        let notes: SharedString = if release.notes.trim().is_empty() {
            strings.update_no_notes.into()
        } else {
            release.notes.clone().into()
        };
        let title = strings
            .update_notes_title
            .replace("{version}", &release.version.to_string());
        let close_label = strings.close;
        let update_label = strings.update_now;
        let fade = cx.theme().background;
        window.open_dialog(cx, move |dialog, window, _| {
            let entity = entity.clone();
            dialog
                .title(dialog_title(title.clone(), close_label))
                .close_button(false)
                .w(dialog_width(DialogSize::STANDARD, window))
                .child(
                    FadeScroll::new("release-notes", fade)
                        .max_h(dialog_body_height(DialogSize::STANDARD, window))
                        .text_sm()
                        .child(notes.clone()),
                )
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("notes-close")
                                .outline()
                                .label(close_label)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("notes-update")
                                .primary()
                                .label(update_label)
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    entity
                                        .update(cx, |this, cx| this.start_update(window, cx))
                                        .ok();
                                }),
                        ),
                )
        });
    }

    pub(crate) fn skip_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(release) = self.available.take() else {
            return;
        };
        let version = release.version.to_string();
        self.settings.updates.skipped_version = Some(version.clone());
        self.save_settings(window, cx);
        window.push_notification(
            Notification::info(
                self.strings()
                    .update_skipped_body
                    .replace("{version}", &version),
            )
            .title(self.strings().update_skipped_title),
            cx,
        );
        cx.notify();
    }

    /// Downloads and verifies the update with a progress dialog.
    pub(crate) fn start_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.updating {
            return;
        }
        let Some(release) = self.available.clone() else {
            return;
        };
        let kind = updater::detect_kind();
        if release.asset_for(kind).is_none() {
            window.push_notification(
                Notification::error(self.strings().update_no_asset)
                    .title(self.strings().update_failed_title),
                cx,
            );
            return;
        }
        self.updating = true;
        cx.notify();

        let progress = Arc::new(DownloadProgress::default());
        let strings = self.strings();
        let view = cx.new(|_| DownloadView {
            version: release.version.to_string(),
            progress: progress.clone(),
            strings,
        });
        let dialog_title = strings.update_updating_title;
        let cancel_label = strings.cancel;
        window.open_dialog(cx, {
            let progress = progress.clone();
            let view = view.clone();
            move |dialog, window, _| {
                let progress = progress.clone();
                dialog
                    .title(dialog_title)
                    .w(dialog_width(DialogSize::COMPACT, window))
                    .close_button(false)
                    .overlay_closable(false)
                    .keyboard(false)
                    .child(view.clone())
                    .footer(
                        h_flex().w_full().justify_end().child(
                            Button::new("update-cancel")
                                .outline()
                                .label(cancel_label)
                                .on_click(move |_, window, cx| {
                                    progress.cancel.store(true, Ordering::Relaxed);
                                    window.close_dialog(cx);
                                }),
                        ),
                    )
            }
        });

        let view = view.downgrade();
        self.update_ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if !this.update(cx, |this, _| this.updating).unwrap_or(false) {
                    break;
                }
                if view.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        }));

        let job = {
            let release = release.clone();
            let progress = progress.clone();
            cx.background_executor()
                .spawn(async move { updater::download(&release, kind, &progress) })
        };
        cx.spawn_in(window, async move |this, cx| {
            let result = job.await;
            this.update_in(cx, |this, window, cx| {
                this.finish_download(result, &progress, &release, kind, window, cx);
            })
            .ok();
        })
        .detach();
    }

    fn finish_download(
        &mut self,
        result: Result<std::path::PathBuf, UpdateError>,
        progress: &DownloadProgress,
        release: &Release,
        kind: InstallKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.updating = false;
        // The Cancel button already closed the dialog.
        let cancelled = progress.cancel.load(Ordering::Relaxed);
        if !cancelled {
            window.close_dialog(cx);
        }
        cx.notify();

        let path = match result {
            Ok(path) if !cancelled => path,
            Ok(_) | Err(UpdateError::Cancelled) => {
                updater::discard_downloads();
                window.push_notification(
                    Notification::info(self.strings().update_cancelled_body),
                    cx,
                );
                return;
            }
            Err(error) => {
                window.push_notification(
                    Notification::error(error.to_string())
                        .title(self.strings().update_failed_title),
                    cx,
                );
                return;
            }
        };

        if self.timer.is_active() {
            let entity = cx.entity().downgrade();
            let alert_title = self.strings().update_restart_title;
            let alert_body = self
                .strings()
                .update_restart_body
                .replace("{version}", &release.version.to_string());
            let alert_ok = self.strings().update_restart_now;
            let alert_cancel = self.strings().update_later;
            window.open_alert_dialog(cx, move |alert, _, _| {
                let entity = entity.clone();
                let path = path.clone();
                alert
                    .title(alert_title)
                    .description(alert_body.clone())
                    .confirm()
                    .button_props(
                        DialogButtonProps::default()
                            .ok_text(alert_ok)
                            .cancel_text(alert_cancel)
                            .on_cancel(|_, _, _| {
                                updater::discard_downloads();
                                true
                            }),
                    )
                    .on_ok(move |_, window, cx| {
                        let path = path.clone();
                        entity
                            .update(cx, |this, cx| this.apply_update(&path, kind, window, cx))
                            .ok();
                        true
                    })
            });
        } else {
            self.apply_update(&path, kind, window, cx);
        }
    }

    fn apply_update(
        &mut self,
        path: &std::path::Path,
        kind: InstallKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match updater::apply(path, kind) {
            Ok(()) => cx.quit(),
            Err(error) => {
                updater::discard_downloads();
                window.push_notification(
                    Notification::error(
                        self.strings()
                            .update_install_failed
                            .replace("{error}", &error.to_string()),
                    )
                    .title(self.strings().update_failed_title),
                    cx,
                );
            }
        }
    }
}
