//! Official WCA random-state scrambles, generated off the UI thread.

use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use tnoodle::puzzle::CubePuzzle;
use tnoodle::{PuzzleRegistry, Scrambler};

use crate::event::WcaEvent;

/// How long `TNoodle` keeps searching for shorter 3x3x3 solutions after finding one.
const TNOODLE_MIN_SEARCH_TIME: Duration = Duration::from_millis(200);

static ACCURATE_3X3: AtomicBool = AtomicBool::new(false);

static ACCURATE_THREE: LazyLock<Scrambler> = LazyLock::new(|| {
    Scrambler::Cube(CubePuzzle::three_by_three().with_min_search_time(TNOODLE_MIN_SEARCH_TIME))
});

/// Sets whether 3x3x3 scrambles keep searching for shorter solutions like `TNoodle`.
pub fn set_accurate_3x3(accurate: bool) {
    ACCURATE_3X3.store(accurate, Ordering::Relaxed);
}

/// Generates a scramble on the calling thread. Large puzzles can take seconds.
pub fn generate(event: WcaEvent) -> String {
    scrambler(event).generate_scramble()
}

/// Prepares a scramble for centered, monospaced display.
///
/// Megaminx rows end in `U` or `U'`; the lone `U` is padded with a no-break
/// space so the `U` column stays aligned and the prime hangs to its right.
/// Other events are returned unchanged. Copy the original, not this text.
#[must_use]
pub fn display_text(event: WcaEvent, scramble: &str) -> String {
    if event != WcaEvent::Megaminx {
        return scramble.to_owned();
    }
    scramble
        .split('\n')
        .map(|line| {
            let line = line.trim_end();
            if line.ends_with('U') {
                format!("{line}\u{a0}")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A drawing of the puzzle as a scramble leaves it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preview {
    /// An SVG document, rasterized at [`PREVIEW_SCALE`] times its natural size
    /// so that it stays sharp on high-density screens.
    pub svg: String,
    /// The natural width in logical pixels.
    pub width: u32,
    /// The natural height in logical pixels.
    pub height: u32,
}

/// How many times larger than its natural size the SVG is rasterized.
pub const PREVIEW_SCALE: u32 = 2;

/// Draws the puzzle in the state `scramble` reaches, or `None` when the
/// scramble is not valid for `event`.
///
/// `TNoodle` draws all twelve events, so this never depends on the event.
pub fn preview(event: WcaEvent, scramble: &str) -> Option<Preview> {
    let mut svg = scrambler(event)
        .draw_scramble(Some(scramble.trim()), None)
        .ok()?;
    let size = svg.size();
    let (width, height) = (
        u32::try_from(size.width).ok()?,
        u32::try_from(size.height).ok()?,
    );
    if width == 0 || height == 0 {
        return None;
    }
    // The view box keeps the drawing in proportion while the document grows.
    svg.root_mut()
        .set_attribute("width", format!("{}px", width * PREVIEW_SCALE));
    svg.root_mut()
        .set_attribute("height", format!("{}px", height * PREVIEW_SCALE));
    Some(Preview {
        svg: svg.to_string(),
        width,
        height,
    })
}

fn scrambler(event: WcaEvent) -> &'static Scrambler {
    if event == WcaEvent::Cube3x3 && ACCURATE_3X3.load(Ordering::Relaxed) {
        return &ACCURATE_THREE;
    }
    let puzzle = match event {
        WcaEvent::Cube2x2 => PuzzleRegistry::Two,
        WcaEvent::Cube3x3 => PuzzleRegistry::Three,
        WcaEvent::Cube4x4 => PuzzleRegistry::Four,
        WcaEvent::Cube5x5 => PuzzleRegistry::Five,
        WcaEvent::Cube6x6 => PuzzleRegistry::Six,
        WcaEvent::Cube7x7 => PuzzleRegistry::Seven,
        WcaEvent::Megaminx => PuzzleRegistry::Mega,
        WcaEvent::Pyraminx => PuzzleRegistry::Pyra,
        WcaEvent::Fto => PuzzleRegistry::Fto,
        WcaEvent::Skewb => PuzzleRegistry::Skewb,
        WcaEvent::Square1 => PuzzleRegistry::Sq1,
        WcaEvent::Clock => PuzzleRegistry::Clock,
    };
    puzzle.scrambler()
}

/// Keeps one scramble ready for a single event on a background thread.
///
/// The worker stops once the queue is dropped, so switching events is just
/// replacing the queue.
pub struct ScrambleQueue {
    event: WcaEvent,
    ready: Receiver<String>,
}

impl ScrambleQueue {
    /// Starts generating scrambles for `event`.
    pub fn new(event: WcaEvent) -> Self {
        // A rendezvous channel: the worker prepares one scramble ahead, then
        // blocks until it is taken or the queue is dropped.
        let (sender, ready) = mpsc::sync_channel(0);
        thread::spawn(move || while sender.send(generate(event)).is_ok() {});
        Self { event, ready }
    }

    pub const fn event(&self) -> WcaEvent {
        self.event
    }

    /// Takes the prepared scramble, if there is one yet.
    pub fn try_next(&self) -> Option<String> {
        self.ready.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn megaminx_rows_get_the_same_width() {
        let text = display_text(WcaEvent::Megaminx, "R++ D-- U\nR-- D++ U'");
        let widths: Vec<_> = text.split('\n').map(|l| l.chars().count()).collect();
        assert_eq!(widths, [10, 10]);
        assert!(text.starts_with("R++ D-- U\u{a0}\n"));
    }

    #[test]
    fn other_events_are_not_padded() {
        assert_eq!(display_text(WcaEvent::Cube3x3, "R U"), "R U");
    }

    #[test]
    fn every_event_generates_a_scramble_that_unsolves_its_puzzle() {
        for event in WcaEvent::ALL {
            let scramble = generate(event);
            let solved = scrambler(event).is_solved_by(&scramble).unwrap();
            assert!(!solved, "{event:?}: {scramble}");
        }
    }

    #[test]
    fn every_event_draws_its_scramble() {
        for event in WcaEvent::ALL {
            let scramble = generate(event);
            let preview = preview(event, &scramble).unwrap_or_else(|| panic!("{event:?}"));
            assert!(preview.svg.starts_with("<svg"), "{event:?}");
            assert!(preview.width > 0 && preview.height > 0, "{event:?}");
            assert!(
                preview
                    .svg
                    .contains(&format!("width=\"{}px\"", preview.width * PREVIEW_SCALE)),
                "{event:?}"
            );
            assert!(preview.svg.contains("viewBox=\"0 0 "), "{event:?}");
        }
    }

    #[test]
    fn a_scramble_for_another_event_has_no_preview() {
        assert!(preview(WcaEvent::Cube3x3, "R U R' U'").is_some());
        assert!(preview(WcaEvent::Cube3x3, "R++ D--").is_none());
        assert!(preview(WcaEvent::Cube3x3, "nonsense").is_none());
    }

    #[test]
    fn the_queue_delivers_scrambles_for_its_event() {
        let queue = ScrambleQueue::new(WcaEvent::Cube2x2);
        let scramble = (0..1000).find_map(|_| {
            thread::sleep(Duration::from_millis(10));
            queue.try_next()
        });
        assert!(scramble.is_some());
    }
}
