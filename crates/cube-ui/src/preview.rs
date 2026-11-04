//! The picture of a puzzle as a scramble leaves it, shown on the timer and in
//! the solve details.

use std::cell::RefCell;
use std::sync::Arc;

use cube_core::WcaEvent;
use cube_core::scramble;
use gpui_kit::*;

/// A rasterizable drawing and its natural size in logical pixels.
#[derive(Clone)]
pub(crate) struct PreviewImage {
    image: Arc<Image>,
    width: f32,
    height: f32,
}

impl PreviewImage {
    /// Draws `scramble` for `event`; `None` when it is not valid for the event.
    fn draw(event: WcaEvent, scramble_text: &str) -> Option<Self> {
        let preview = scramble::preview(event, scramble_text)?;
        Some(Self {
            image: Arc::new(Image::from_bytes(
                ImageFormat::Svg,
                preview.svg.into_bytes(),
            )),
            width: preview.width as f32,
            height: preview.height as f32,
        })
    }

    /// The drawing as tall as `max_height` allows, never beyond the size it is
    /// rasterized at and never wider than the space it is placed in.
    pub(crate) fn element(&self, max_height: Pixels) -> impl IntoElement + use<> {
        let scale = (max_height / px(self.height)).min(scramble::PREVIEW_SCALE as f32);
        img(self.image.clone())
            .w(px(self.width * scale))
            .h(px(self.height * scale))
            .max_w_full()
            .object_fit(ObjectFit::Contain)
    }
}

/// Remembers the drawing of the latest scramble, since the views that show it
/// render far more often than the scramble changes.
#[derive(Default)]
pub(crate) struct PreviewCache(RefCell<Option<Entry>>);

struct Entry {
    event: WcaEvent,
    scramble: String,
    image: Option<PreviewImage>,
}

impl PreviewCache {
    /// Returns the drawing for the scramble, drawing it only when it differs
    /// from the last request.
    pub(crate) fn get(&self, event: WcaEvent, scramble_text: &str) -> Option<PreviewImage> {
        let mut entry = self.0.borrow_mut();
        if let Some(entry) = entry.as_ref()
            && entry.event == event
            && entry.scramble == scramble_text
        {
            return entry.image.clone();
        }
        let image = PreviewImage::draw(event, scramble_text);
        *entry = Some(Entry {
            event,
            scramble: scramble_text.to_owned(),
            image: image.clone(),
        });
        image
    }
}
