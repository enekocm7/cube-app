//! The picture of a puzzle as a scramble leaves it, shown on the timer and in
//! the solve details: the flat net, or a solid to turn around.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use cube_core::preview3d::{self, Drag, Grip, Orientation, Solid, Spin};
use cube_core::scramble;
use cube_core::{Action, WcaEvent};
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

    /// The drawing as tall as `max_height` allows, never scaled past
    /// `max_scale` or the size it is rasterized at, and never wider than the
    /// space it is placed in.
    fn element(&self, max_height: Pixels, max_scale: f32) -> impl IntoElement + use<> {
        let scale = (max_height / px(self.height))
            .min(max_scale)
            .min(scramble::PREVIEW_SCALE as f32);
        img(self.image.clone())
            .w(px(self.width * scale))
            .h(px(self.height * scale))
            .max_w_full()
            .object_fit(ObjectFit::Contain)
    }
}

/// How a 3D preview is held, shared by the view that draws it and whoever
/// turns it from the keyboard. Cloning shares the same puzzle in the hand.
#[derive(Clone, Default)]
pub(crate) struct Turntable {
    held: Rc<Cell<Orientation>>,
    /// The press on the view the pointer is still holding, if any.
    press: Rc<Cell<Option<Press>>>,
}

/// A button held down on a 3D preview.
#[derive(Clone, Copy)]
struct Press {
    button: MouseButton,
    /// Where the pointer last was.
    last: Point<Pixels>,
    /// Whether the pointer has moved far enough since the press for it to
    /// be a drag rather than a click.
    moved: bool,
    drag: Drag,
}

impl Press {
    /// A press of `button` at `position`. As in most 3D viewers, the left
    /// button turns the puzzle freely, or along one axis with `Shift`; the
    /// right button, or the left with `Alt`, rolls it like a dial.
    fn new(button: MouseButton, position: Point<Pixels>, modifiers: Modifiers) -> Self {
        let grip = if button == MouseButton::Right || modifiers.alt {
            Grip::Roll
        } else if modifiers.shift {
            Grip::Straight
        } else {
            Grip::Free
        };
        Self {
            button,
            last: position,
            moved: false,
            drag: Drag::new(grip),
        }
    }
}

impl Turntable {
    /// Runs `action` if it turns or resets the preview. Returns whether it
    /// was one of those, and the view needs drawing again.
    pub(crate) fn act(&self, action: Action) -> bool {
        let spin = match action {
            Action::RotatePreviewLeft => Spin::Left,
            Action::RotatePreviewRight => Spin::Right,
            Action::RotatePreviewUp => Spin::Up,
            Action::RotatePreviewDown => Spin::Down,
            Action::RollPreviewLeft => Spin::RollLeft,
            Action::RollPreviewRight => Spin::RollRight,
            Action::ResetPreviewRotation => {
                self.held.set(Orientation::STANDARD);
                return true;
            }
            _ => return false,
        };
        self.held.set(self.held.get().spun(spin));
        true
    }
}

/// How far the pointer may wander between press and release for it to still
/// be a click.
const CLICK_SLOP: f32 = 4.;

/// Called when a press on a preview is let go without a drag.
pub(crate) type OnClick = Rc<dyn Fn(&mut Window, &mut App)>;

/// A solid drawn in a square of `side`, to turn by dragging: freely, along
/// one axis with `Shift`, or rolled with the right button or `Alt`. A left
/// click that does not drag calls `clicked`.
///
/// The view keeps nothing of its own: `turntable` holds the orientation, so
/// the keyboard turns the same puzzle, and a new `solid` is drawn the way the
/// last one was held. A press on it still reaches what lies under it, unless
/// the caller adds `block_mouse_except_scroll`.
pub(crate) fn solid_view(
    id: impl Into<ElementId>,
    solid: Rc<Solid>,
    turntable: &Turntable,
    side: impl Clone + Into<Length>,
    clicked: OnClick,
) -> Stateful<Div> {
    let painted = turntable.clone();
    let pressed = turntable.clone();
    div()
        .id(id)
        .flex_none()
        .size(side)
        .max_w_full()
        .cursor(CursorStyle::OpenHand)
        .on_mouse_down(MouseButton::Left, {
            let pressed = pressed.clone();
            move |event, _, _| {
                let press = Press::new(MouseButton::Left, event.position, event.modifiers);
                pressed.press.set(Some(press));
            }
        })
        .on_mouse_down(MouseButton::Right, move |event, _, _| {
            let press = Press::new(MouseButton::Right, event.position, event.modifiers);
            pressed.press.set(Some(press));
        })
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    paint_solid(&solid, &painted, bounds, &clicked, window);
                },
            )
            .size_full(),
        )
}

/// Fills the polygons of `solid` and follows a drag that began on it, even
/// once the pointer has left its square. A press let go where it began calls
/// `clicked`.
fn paint_solid(
    solid: &Solid,
    turntable: &Turntable,
    bounds: Bounds<Pixels>,
    clicked: &OnClick,
    window: &mut Window,
) {
    let side = bounds.size.width.min(bounds.size.height);
    if side <= px(0.) {
        return;
    }
    let half = side / 2.;
    let middle = bounds.center();

    // Neighbours of one colour share a path: they never overlap, and a path
    // is the costly part of painting.
    let mut batch: Option<(u32, PathBuilder)> = None;
    let flush = |batch: Option<(u32, PathBuilder)>, window: &mut Window| {
        if let Some((colour, builder)) = batch
            && let Ok(path) = builder.build()
        {
            window.paint_path(path, rgb(colour));
        }
    };
    for polygon in solid.project(turntable.held.get()) {
        let colour = polygon.colour.rgb();
        if batch.as_ref().is_none_or(|(current, _)| *current != colour) {
            flush(batch.take(), window);
            batch = Some((colour, PathBuilder::fill()));
        }
        let points: Vec<Point<Pixels>> = polygon
            .points
            .iter()
            .map(|&[x, y]| point(middle.x + half * x, middle.y + half * y))
            .collect();
        if let Some((_, builder)) = batch.as_mut() {
            builder.add_polygon(&points, true);
        }
    }
    flush(batch, window);

    let view = window.current_view();
    let moved = turntable.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
        if phase != DispatchPhase::Bubble {
            return;
        }
        let Some(mut press) = moved.press.get() else {
            return;
        };
        if event.pressed_button != Some(press.button) {
            // The button was let go where no release could be seen.
            moved.press.set(None);
            return;
        }
        if !press.moved {
            // A hand never holds still: small slips are part of a click.
            let gap = event.position - press.last;
            if gap.x.abs().max(gap.y.abs()) < px(CLICK_SLOP) {
                return;
            }
            press.moved = true;
        }
        // From the middle of the view, in times its size.
        let at = |position: Point<Pixels>| {
            [
                f64::from((position.x - middle.x) / side),
                f64::from((position.y - middle.y) / side),
            ]
        };
        let held = moved.held.get();
        let turned = press.drag.turn(held, at(press.last), at(event.position));
        moved.held.set(turned);
        press.last = event.position;
        moved.press.set(Some(press));
        cx.notify(view);
    });
    let released = turntable.clone();
    let clicked = clicked.clone();
    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble {
            return;
        }
        let Some(press) = released.press.get() else {
            return;
        };
        if event.button != press.button {
            return;
        }
        released.press.set(None);
        if press.button == MouseButton::Left && !press.moved {
            clicked(window, cx);
        }
    });
}

/// What the preview shows for a scramble.
#[derive(Clone)]
pub(crate) enum PreviewDrawing {
    /// The flat net, and whether a click on it can show the solid instead.
    Net(PreviewImage, bool),
    /// The puzzle in 3D.
    Solid(Rc<Solid>),
}

impl PreviewDrawing {
    /// Draws `scramble` for `event`, in 3D when `three_d` asks for it and as
    /// the net otherwise, or when the text only makes sense to the net.
    /// `None` when the text is not a scramble for the event.
    fn draw(event: WcaEvent, scramble_text: &str, three_d: bool) -> Option<Self> {
        if three_d && let Some(solid) = preview3d::solid(event, scramble_text) {
            return Some(Self::Solid(Rc::new(solid)));
        }
        // A net shown because the solid could not be built stays a net.
        PreviewImage::draw(event, scramble_text).map(|image| Self::Net(image, !three_d))
    }

    /// The drawing as tall as `max_height` allows and never scaled past
    /// `max_scale`. A solid takes the height its net would, and is turned
    /// with `turntable`; `id` tells its views apart. A click on the net, or
    /// on the solid without dragging it, calls `switch`, which swaps the net
    /// and the solid. With `covering`, a press on the drawing never reaches
    /// what lies under it.
    pub(crate) fn element(
        &self,
        id: &'static str,
        max_height: Pixels,
        max_scale: f32,
        turntable: &Turntable,
        covering: bool,
        switch: impl Fn(&mut Window, &mut App) + 'static,
    ) -> AnyElement {
        let view = match self {
            Self::Net(image, clickable) => {
                let net = div()
                    .id(id)
                    .flex_none()
                    .max_w_full()
                    .child(image.element(max_height, max_scale));
                if *clickable {
                    net.cursor_pointer()
                        .on_click(move |_, window, cx| switch(window, cx))
                } else {
                    net
                }
            }
            Self::Solid(solid) => {
                let natural = px(solid.net_height() as f32 * max_scale);
                let side = max_height.min(natural);
                solid_view(id, solid.clone(), turntable, side, Rc::new(switch))
            }
        };
        if covering {
            view.block_mouse_except_scroll().into_any_element()
        } else {
            view.into_any_element()
        }
    }
}

/// Remembers the drawing of the latest scramble, since the views that show it
/// render far more often than the scramble changes.
#[derive(Default)]
pub(crate) struct PreviewCache(RefCell<Option<Entry>>);

struct Entry {
    event: WcaEvent,
    scramble: String,
    three_d: bool,
    drawing: Option<PreviewDrawing>,
}

impl PreviewCache {
    /// Returns the drawing for the scramble, drawing it only when it differs
    /// from the last request.
    pub(crate) fn get(
        &self,
        event: WcaEvent,
        scramble_text: &str,
        three_d: bool,
    ) -> Option<PreviewDrawing> {
        let mut entry = self.0.borrow_mut();
        if let Some(entry) = entry.as_ref()
            && entry.event == event
            && entry.three_d == three_d
            && entry.scramble == scramble_text
        {
            return entry.drawing.clone();
        }
        let drawing = PreviewDrawing::draw(event, scramble_text, three_d);
        *entry = Some(Entry {
            event,
            scramble: scramble_text.to_owned(),
            three_d,
            drawing: drawing.clone(),
        });
        drawing
    }
}
