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
    fn every_event_generates_a_scramble_that_unsolves_its_puzzle() {
        for event in WcaEvent::ALL {
            let scramble = generate(event);
            let solved = scrambler(event).is_solved_by(&scramble).unwrap();
            assert!(!solved, "{event:?}: {scramble}");
        }
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
