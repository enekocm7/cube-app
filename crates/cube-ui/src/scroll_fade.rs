//! Soft fades over the edges of a scroll region that still has content past
//! them.
//!
//! A scroll viewport clips its content hard: when a list continues past the
//! edge, the next row is cut in half and nothing says there is more to scroll.
//! These fades paint a short gradient of the surface colour over an edge, and
//! only while content is actually hidden past it, so a region that fits (or
//! sits at one of its ends) shows no gradient at all.
//!
//! [`FadeScroll`] is the reusable scroll region for a column of items: it owns
//! its scroll handle, adds the fades and the scrollbar, and accepts the same
//! styles and children as a `v_flex`. [`fading_scroll`] does the same for
//! lists that already track their own handle, such as a `uniform_list`.

use std::rc::Rc;

use gpui_kit::component::scroll::{ScrollableMask, Scrollbar, ScrollbarHandle};
use gpui_kit::component::{StyledExt as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// How tall a fully revealed edge fade gets.
const FADE_HEIGHT: Pixels = px(24.);
/// Offsets closer than this count as being at an edge, so the sub-pixel
/// residue left by clamping does not paint a fade over no hidden content.
const EDGE_EPSILON: Pixels = px(0.5);

/// A vertically scrolling column that fades the edge past which content
/// continues.
///
/// It lays out like the `v_flex` it replaces: padding, gaps, text styles and
/// event handlers apply to the column of children, while the size and flex
/// properties size the scroll region as a whole. Inside a `relative` parent the
/// fades and scrollbar overlay the region.
#[derive(IntoElement)]
pub(crate) struct FadeScroll {
    id: ElementId,
    content: Div,
    color: Hsla,
    nested: bool,
}

impl FadeScroll {
    /// A scroll region keyed by `id`; use one unique id per region.
    pub(crate) fn new(id: impl Into<ElementId>, color: impl Into<Hsla>) -> Self {
        Self {
            id: id.into(),
            content: v_flex(),
            color: color.into(),
            nested: false,
        }
    }

    /// Keep wheel scrolling inside this region when it sits in another
    /// scroller, handing the wheel over only once this one reaches an edge.
    pub(crate) fn nested(mut self) -> Self {
        self.nested = true;
        self
    }
}

impl Styled for FadeScroll {
    fn style(&mut self) -> &mut StyleRefinement {
        self.content.style()
    }
}

impl ParentElement for FadeScroll {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.content.extend(elements);
    }
}

impl InteractiveElement for FadeScroll {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.content.interactivity()
    }
}

impl StatefulInteractiveElement for FadeScroll {}

impl RenderOnce for FadeScroll {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // The scroll position outlives a frame, so the handle lives in the
        // window's keyed state under this component's position and id.
        let handle = window
            .use_keyed_state((self.id.clone(), "scroll"), cx, |_, _| {
                ScrollHandle::default()
            })
            .read(cx)
            .clone();

        // The region owns the size; the content is free to grow past it so it
        // can scroll. This mirrors what `overflow_y_scrollbar` does.
        let region_style = region_style(self.content.style());
        self.content.style().max_size.height = None;
        let content = self.content.flex_none().h_auto().min_h_full();

        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .refine_style(&region_style)
            .child(
                div()
                    .id((self.id.clone(), "viewport"))
                    .size_full()
                    .track_scroll(&handle)
                    .overflow_y_scroll()
                    .child(content),
            )
            .child(ScrollFade::new(&handle, self.color))
            .when(self.nested, |region| {
                region.child(
                    ScrollableMask::new(Axis::Vertical, &handle).id((self.id.clone(), "wheel")),
                )
            })
            .child(Scrollbar::vertical(&handle).id((self.id.clone(), "scrollbar")))
    }
}

/// The layout of the scroll region as a whole, copied from the content so it
/// participates in its parent's layout the way the styled column would.
fn region_style(style: &StyleRefinement) -> StyleRefinement {
    StyleRefinement {
        size: style.size.clone(),
        min_size: style.min_size.clone(),
        max_size: style.max_size.clone(),
        flex_grow: style.flex_grow,
        flex_shrink: style.flex_shrink,
        flex_basis: style.flex_basis,
        align_self: style.align_self,
        // A scroll region must not inherit its content's automatic minimum.
        overflow: PointRefinement {
            x: None,
            y: Some(Overflow::Hidden),
        },
        ..Default::default()
    }
}

/// Wraps a list that tracks its own scroll handle, such as a `uniform_list`,
/// and fades the edge past which its content continues.
///
/// Size the returned wrapper (e.g. `size_full` inside a sized panel) so the
/// list has a viewport, and add the scrollbar outside it so the bar paints
/// above the fade.
pub(crate) fn fading_scroll<E, H>(list: E, handle: &H, color: impl Into<Hsla>) -> Div
where
    E: IntoElement,
    H: ScrollbarHandle + Clone + 'static,
{
    div()
        .relative()
        .child(list)
        .child(ScrollFade::new(handle, color))
}

/// Fades the top and bottom edges of a scroll region while content is hidden
/// past them.
pub(crate) struct ScrollFade {
    handle: Rc<dyn ScrollbarHandle>,
    color: Hsla,
}

impl ScrollFade {
    /// Fades the region tracked by `handle`. Add the fade after the region's
    /// content, inside a `relative` parent.
    pub(crate) fn new<H: ScrollbarHandle + Clone + 'static>(
        handle: &H,
        color: impl Into<Hsla>,
    ) -> Self {
        Self {
            handle: Rc::new(handle.clone()),
            color: color.into(),
        }
    }
}

impl IntoElement for ScrollFade {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ScrollFade {
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
        // Absolute and parent-sized, like the masks in gpui-base: the fade
        // must cover the viewport and not take part in its layout.
        let mut style = Style {
            position: Position::Absolute,
            ..Default::default()
        };
        style.inset.top = px(0.).into();
        style.inset.left = px(0.).into();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();

        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        _: &mut App,
    ) {
        let viewport = self.handle.viewport_bounds().size.height;
        let hidden = self.handle.content_size().height - viewport;
        if hidden <= EDGE_EPSILON {
            return;
        }

        // Both offsets are measured from the top edge: 0 at the top and
        // `-hidden` at the bottom. `clamp` also absorbs overscroll.
        let offset = self.handle.offset().y.clamp(-hidden, px(0.));
        // A short viewport cannot fit a full-height fade on each edge.
        let fade_height = FADE_HEIGHT.min(bounds.size.height / 2.);

        let top_hidden = -offset;
        if top_hidden > EDGE_EPSILON {
            let height = fade_height.min(top_hidden);
            paint_fade(
                window,
                Bounds::new(bounds.origin, size(bounds.size.width, height)),
                0.,
                self.color,
            );
        }

        let bottom_hidden = hidden + offset;
        if bottom_hidden > EDGE_EPSILON {
            let height = fade_height.min(bottom_hidden);
            let origin = point(
                bounds.origin.x,
                bounds.origin.y + bounds.size.height - height,
            );
            paint_fade(
                window,
                Bounds::new(origin, size(bounds.size.width, height)),
                180.,
                self.color,
            );
        }
    }
}

/// Paints one edge fade, opaque at the viewport edge and clear toward the
/// content. `angle` is 0 for the top edge and 180 for the bottom.
fn paint_fade(window: &mut Window, bounds: Bounds<Pixels>, angle: f32, color: Hsla) {
    window.paint_quad(PaintQuad {
        bounds,
        corner_radii: Corners::all(px(0.)),
        background: linear_gradient(
            angle,
            linear_color_stop(color.opacity(0.), 0.),
            linear_color_stop(color, 1.),
        ),
        border_widths: Edges::all(px(0.)),
        border_color: color.opacity(0.),
        border_style: BorderStyle::default(),
    });
}
