//! The image preview: a solve or an average as a card, with its link.
//!
//! `cube_core::share_card` lays the card out and rasterizes it; this module
//! only runs that off the UI thread, shows the result and hands it to the
//! clipboard or a file. Nothing leaves the machine.

use std::rc::Rc;
use std::sync::Arc;

use cube_core::settings::FontChoice;
use cube_core::share_card::{self, CardFont, CardLabels, CardStyle, ShareCard};
use cube_core::share_link;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, WindowExt as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::CubeApp;
use crate::dialog_size::{DialogSize, dialog_body, dialog_body_height, dialog_title, dialog_width};
use crate::dialog_submit::{DialogSubmit as _, Submit, close_action, on_press};
use crate::palette;
use crate::scroll_fade::FadeScroll;

const DIALOG_SIZE: DialogSize = DialogSize::CARD_VIEWER;
/// The room the dialog's own padding takes from its width.
const DIALOG_PADDING: Pixels = px(48.);

/// The finished card, as the preview shows it.
struct ShareView {
    app: Entity<CubeApp>,
    image: Arc<Image>,
    file_name: String,
}

impl ShareView {
    /// Opens a save picker and writes the card where it says.
    fn save(&self, window: &mut Window, cx: &mut Context<Self>) {
        let directory = dirs::download_dir()
            .or_else(dirs::picture_dir)
            .or_else(dirs::home_dir)
            .unwrap_or_default();
        let receiver = cx.prompt_for_new_path(&directory, Some(&self.file_name));
        let image = self.image.clone();
        let app = self.app.clone();
        cx.spawn_in(window, async move |_, cx| {
            // `None` is a cancelled picker, or a window closed under it.
            let picked = match receiver.await {
                Ok(Ok(Some(path))) => {
                    share_card::save_png(&path, image.bytes()).map_err(|error| format!("{error:#}"))
                }
                Ok(Err(error)) => Err(format!("{error:#}")),
                Ok(Ok(None)) | Err(_) => return,
            };
            let _ = app.update_in(cx, |app, window, cx| {
                let strings = app.strings();
                match picked {
                    Ok(path) => {
                        let path = path.display().to_string();
                        let message = strings.share_image_saved.replace("{path}", &path);
                        app.toast(&message, window, cx);
                    }
                    Err(error) => {
                        let message = strings.share_save_failed.replace("{error}", &error);
                        window.push_notification(Notification::error(message), cx);
                    }
                }
            });
        })
        .detach();
    }
}

impl Render for ShareView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (background, border) = (theme.background, theme.border);
        let max_height = dialog_body_height(DIALOG_SIZE, window);
        let aspect = share_card::WIDTH as f32 / share_card::HEIGHT as f32;
        let width = (dialog_width(DIALOG_SIZE, window) - DIALOG_PADDING).min(max_height * aspect);
        dialog_body(
            FadeScroll::new("share-card", background)
                .max_h(max_height)
                .items_center()
                .child(
                    h_flex()
                        .w(width)
                        .h(width / aspect)
                        .max_w_full()
                        .rounded_lg()
                        .overflow_hidden()
                        .border_1()
                        .border_color(border)
                        .child(
                            img(self.image.clone())
                                .size_full()
                                .object_fit(ObjectFit::Contain),
                        ),
                ),
        )
    }
}

impl CubeApp {
    /// Creates the image of the target solve.
    pub(crate) fn share_target_solve(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.target_solve() {
            self.share_solve(index, window, cx);
        }
    }

    /// Creates the image of the solve at `index` of the session.
    pub(crate) fn share_solve(&self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(solve) = self.session().solves().get(index) else {
            return;
        };
        let strings = self.strings();
        let date = self.language().format_date(solve.solved_at_unix_ms());
        let is_best = self.stats.count > 1
            && self
                .stats
                .best
                .is_some_and(|(best, _)| solve.effective_ms() == Some(best));
        let labels = CardLabels {
            title: strings.stat_single,
            date: &date,
            badge: is_best.then_some(strings.share_session_best),
        };
        let card = ShareCard::single(solve, labels);
        self.open_share(&card, "single", share_link::url(solve), window, cx);
    }

    /// Copies the link of the target solve, or says why it has none.
    pub(crate) fn copy_target_solve_link(&self, window: &mut Window, cx: &mut Context<Self>) {
        let solves = self.session().solves();
        let Some(solve) = self.target_solve().and_then(|index| solves.get(index)) else {
            return;
        };
        let strings = self.strings();
        match share_link::url(solve) {
            Some(link) => {
                cx.write_to_clipboard(ClipboardItem::new_string(link));
                self.toast(strings.share_link_copied, window, cx);
            }
            None => self.toast_warning(strings.share_link_unavailable, window, cx),
        }
    }

    /// Draws `card` in the background and opens the preview once it is done.
    ///
    /// `label` names what it shows in the suggested file name.
    pub(crate) fn open_share(
        &self,
        card: &ShareCard,
        label: &str,
        link: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.drawing_card.replace(true) {
            return;
        }
        let svg = card.svg(CardStyle {
            colors: palette::card_colors(cx),
            font: match self.settings.font {
                FontChoice::System => CardFont::System,
                FontChoice::Timer | FontChoice::Times | FontChoice::Whole => CardFont::Bundled,
            },
            language: self.language().code(),
        });
        let file_name = card.file_name(label);
        let drawing = cx
            .background_executor()
            .spawn(async move { share_card::render_png(&svg) });
        cx.spawn_in(window, async move |app, cx| {
            let drawn = drawing.await;
            let _ = app.update_in(cx, |app, window, cx| {
                app.drawing_card.set(false);
                match drawn {
                    // An attempt started meanwhile must not lose its keys to a dialog.
                    Ok(_) if app.timer.is_active() => {}
                    Ok(png) => app.show_card(png, file_name, link, window, cx),
                    Err(error) => {
                        let message = app
                            .strings()
                            .share_failed
                            .replace("{error}", &format!("{error:#}"));
                        window.push_notification(Notification::error(message), cx);
                    }
                }
            });
        })
        .detach();
    }

    /// Opens the preview of a finished card.
    fn show_card(
        &self,
        png: Vec<u8>,
        file_name: String,
        link: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let strings = self.strings();
        let image = Arc::new(Image::from_bytes(ImageFormat::Png, png));
        let app = cx.entity();
        let view = cx.new(|_| ShareView {
            app: app.clone(),
            image: image.clone(),
            file_name,
        });
        window.open_dialog(cx, move |dialog, window, _| {
            // Copying leaves the preview open, for a download after it.
            let copy: Submit = {
                let (image, app) = (image.clone(), app.clone());
                Rc::new(move |window, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_image(&image));
                    app.update(cx, |app, cx| {
                        app.toast(app.strings().share_image_copied, window, cx);
                    });
                })
            };
            let close = close_action();
            let saver = view.clone();
            let link_app = app.clone();
            let link = link.clone();
            dialog
                .title(dialog_title(strings.share_title, strings.close))
                .close_button(false)
                .w(dialog_width(DIALOG_SIZE, window))
                .submit_on_enter(&copy)
                .child(view.clone())
                .footer(
                    h_flex()
                        .w_full()
                        .flex_wrap()
                        .justify_end()
                        .gap_2()
                        .when_some(link, |footer, link| {
                            footer.child(
                                Button::new("share-link")
                                    .ghost()
                                    .label(strings.share_copy_link)
                                    .mr_auto()
                                    .on_click(move |_, window, cx| {
                                        let item = ClipboardItem::new_string(link.clone());
                                        cx.write_to_clipboard(item);
                                        link_app.update(cx, |app, cx| {
                                            app.toast(strings.share_link_copied, window, cx);
                                        });
                                    }),
                            )
                        })
                        .child(
                            Button::new("share-close")
                                .outline()
                                .label(strings.close)
                                .on_click(on_press(&close)),
                        )
                        .child(
                            Button::new("share-save")
                                .outline()
                                .label(strings.share_save_image)
                                .on_click(move |_, window, cx| {
                                    saver.update(cx, |view, cx| view.save(window, cx));
                                }),
                        )
                        .child(
                            Button::new("share-copy")
                                .primary()
                                .label(strings.share_copy_image)
                                .on_click(on_press(&copy)),
                        ),
                )
        });
    }
}
