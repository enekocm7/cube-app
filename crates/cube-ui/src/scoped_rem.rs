//! Drawing part of the interface at a rem size of its own.
//!
//! `Window::with_rem_size` only applies during layout and painting, which a
//! view's `render` never reaches, so it takes an element to hold it.

use gpui_kit::*;

/// Lays out and paints its child with one rem being `rem` pixels.
pub(crate) struct ScopedRem {
    rem: Pixels,
    child: AnyElement,
}

impl ScopedRem {
    pub(crate) fn new(rem: Pixels, child: impl IntoElement) -> Self {
        Self {
            rem,
            child: child.into_any_element(),
        }
    }
}

impl IntoElement for ScopedRem {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ScopedRem {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let child = &mut self.child;
        let layout =
            window.with_rem_size(Some(self.rem), |window| child.request_layout(window, cx));
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let child = &mut self.child;
        window.with_rem_size(Some(self.rem), |window| {
            child.prepaint(window, cx);
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let child = &mut self.child;
        window.with_rem_size(Some(self.rem), |window| child.paint(window, cx));
    }
}
