//! Official WCA random-state scrambles, generated off the UI thread.

use std::collections::{HashMap, HashSet, VecDeque};
use std::future::poll_fn;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex, MutexGuard};
use std::task::{Poll, Waker};
use std::thread;
use std::time::Duration;

use tnoodle::puzzle::CubePuzzle;
use tnoodle::{PuzzleRegistry, Scrambler};

use crate::event::WcaEvent;

/// How long `TNoodle` keeps searching for shorter 3x3x3 solutions after finding one.
const TNOODLE_MIN_SEARCH_TIME: Duration = Duration::from_millis(200);

/// How many scrambles the queue keeps prepared for the current event, so that
/// skipping and then starting an attempt does not wait for a fresh solve.
const SCRAMBLES_AHEAD: usize = 2;

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

/// Keeps scrambles prepared on background threads.
///
/// Only the current event is generated, up to [`SCRAMBLES_AHEAD`] at a time.
/// What was prepared for other events stays available, so switching back to
/// one is instant. Each event gets one worker the first time it is current, so
/// a slow solve never delays another event, and a worker whose event is no
/// longer current sleeps after its running solve instead of starting another.
/// The workers stop once the queue is dropped.
pub struct ScrambleQueue {
    shared: Arc<Shared>,
}

/// What the queue and its worker share.
struct Shared {
    state: Mutex<State>,
    /// Wakes the workers when there may be something to generate.
    work: Condvar,
    generate: Box<dyn Fn(WcaEvent) -> String + Send + Sync>,
}

struct State {
    event: WcaEvent,
    prepared: HashMap<WcaEvent, VecDeque<String>>,
    /// The events that have a worker.
    workers: HashSet<WcaEvent>,
    /// Counts scrambles delivered for the event that was current at the time.
    delivered: u64,
    /// Bumped by [`ScrambleQueue::discard`] to invalidate the running solve.
    generation: u64,
    waker: Option<Waker>,
    closed: bool,
}

impl State {
    fn wants_scramble(&self, event: WcaEvent) -> bool {
        self.event == event
            && self
                .prepared
                .get(&event)
                .is_none_or(|ready| ready.len() < SCRAMBLES_AHEAD)
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().expect("no panics while locked")
    }

    /// Waits until `event` needs a scramble, returning the generation to solve
    /// it for, or `None` once closed.
    fn next_job(&self, event: WcaEvent) -> Option<u64> {
        let mut state = self.lock();
        while !state.closed && !state.wants_scramble(event) {
            state = self.work.wait(state).expect("no panics while locked");
        }
        (!state.closed).then_some(state.generation)
    }

    fn deliver(&self, event: WcaEvent, generation: u64, scramble: String) {
        let mut state = self.lock();
        if state.generation != generation {
            return;
        }
        state.prepared.entry(event).or_default().push_back(scramble);
        if event == state.event {
            state.delivered += 1;
            let waker = state.waker.take();
            drop(state);
            if let Some(waker) = waker {
                waker.wake();
            }
        }
    }

    /// Starts the worker for the current event, unless it already has one.
    fn ensure_worker(self: &Arc<Self>, state: &mut State) {
        let event = state.event;
        if state.workers.insert(event) {
            let shared = Arc::clone(self);
            thread::spawn(move || {
                while let Some(generation) = shared.next_job(event) {
                    let scramble = (shared.generate)(event);
                    shared.deliver(event, generation, scramble);
                }
            });
        }
    }
}

impl ScrambleQueue {
    /// Starts generating scrambles for `event`.
    pub fn new(event: WcaEvent) -> Self {
        Self::with_generator(event, generate)
    }

    fn with_generator(
        event: WcaEvent,
        generate: impl Fn(WcaEvent) -> String + Send + Sync + 'static,
    ) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                event,
                prepared: HashMap::new(),
                workers: HashSet::new(),
                delivered: 0,
                generation: 0,
                waker: None,
                closed: false,
            }),
            work: Condvar::new(),
            generate: Box::new(generate),
        });
        shared.ensure_worker(&mut shared.lock());
        Self { shared }
    }

    /// Takes a prepared scramble for the current event, if there is one yet.
    pub fn try_next(&self) -> Option<String> {
        let mut state = self.shared.lock();
        let event = state.event;
        let scramble = state.prepared.get_mut(&event)?.pop_front();
        if scramble.is_some() {
            self.shared.work.notify_all();
        }
        scramble
    }

    /// Makes `event` current. `unused` is a scramble taken for the previous
    /// event but never attempted: it is kept to be shown again on return.
    pub fn switch_event(&self, event: WcaEvent, unused: Option<String>) {
        let mut state = self.shared.lock();
        if let Some(scramble) = unused {
            let previous = state.event;
            state
                .prepared
                .entry(previous)
                .or_default()
                .push_front(scramble);
        }
        state.event = event;
        self.shared.ensure_worker(&mut state);
        self.shared.work.notify_all();
    }

    /// Forgets everything prepared for `event`, and the solve in progress,
    /// because the way scrambles are generated changed.
    pub fn discard(&self, event: WcaEvent) {
        let mut state = self.shared.lock();
        state.prepared.remove(&event);
        state.generation += 1;
        self.shared.work.notify_all();
    }

    /// Returns a signal for scrambles becoming available.
    pub fn signal(&self) -> ScrambleSignal {
        ScrambleSignal {
            shared: Arc::clone(&self.shared),
            seen: 0,
        }
    }
}

impl Drop for ScrambleQueue {
    fn drop(&mut self) {
        let mut state = self.shared.lock();
        state.closed = true;
        let waker = state.waker.take();
        drop(state);
        self.shared.work.notify_all();
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

/// Lets an async task sleep until the worker has something new, instead of
/// polling [`ScrambleQueue::try_next`] on a timer.
pub struct ScrambleSignal {
    shared: Arc<Shared>,
    seen: u64,
}

impl ScrambleSignal {
    /// Resolves with `true` once a scramble was delivered for the current event
    /// since the last call, or with `false` once the queue is gone. A delivery
    /// is never missed, but it may already have been taken.
    pub async fn changed(&mut self) -> bool {
        poll_fn(|context| {
            let mut state = self.shared.lock();
            if state.closed {
                Poll::Ready(false)
            } else if state.delivered == self.seen {
                state.waker = Some(context.waker().clone());
                Poll::Pending
            } else {
                self.seen = state.delivered;
                Poll::Ready(true)
            }
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::pin;
    use std::sync::atomic::AtomicUsize;
    use std::sync::mpsc;
    use std::task::{Context, Wake};
    use std::time::Instant;

    use super::*;

    const PATIENCE: Duration = Duration::from_secs(10);

    fn eventually<T>(mut check: impl FnMut() -> Option<T>) -> T {
        let start = Instant::now();
        loop {
            if let Some(value) = check() {
                return value;
            }
            assert!(start.elapsed() < PATIENCE, "timed out");
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// A queue over a generator that numbers its scrambles, with the count.
    fn counting_queue(event: WcaEvent) -> (ScrambleQueue, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        let generated = Arc::clone(&count);
        let queue = ScrambleQueue::with_generator(event, move |event| {
            let number = generated.fetch_add(1, Ordering::SeqCst);
            format!("{event:?} {number}")
        });
        (queue, count)
    }

    fn wait_for_count(count: &AtomicUsize, expected: usize) {
        eventually(|| (count.load(Ordering::SeqCst) >= expected).then_some(()));
        // Anything past the expected count would show up right after it.
        thread::sleep(Duration::from_millis(20));
        assert_eq!(count.load(Ordering::SeqCst), expected);
    }

    struct Unpark(thread::Thread);

    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        let waker = Arc::new(Unpark(thread::current())).into();
        let mut context = Context::from_waker(&waker);
        let mut future = pin!(future);
        let start = Instant::now();
        loop {
            if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
                return value;
            }
            assert!(start.elapsed() < PATIENCE, "timed out");
            thread::park_timeout(PATIENCE);
        }
    }

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
    fn the_queue_delivers_scrambles_for_its_event() {
        let queue = ScrambleQueue::new(WcaEvent::Cube2x2);
        let scramble = (0..1000).find_map(|_| {
            thread::sleep(Duration::from_millis(10));
            queue.try_next()
        });
        assert!(scramble.is_some());
    }

    #[test]
    fn the_queue_stays_a_fixed_depth_ahead() {
        let (queue, count) = counting_queue(WcaEvent::Cube3x3);
        wait_for_count(&count, SCRAMBLES_AHEAD);

        assert_eq!(queue.try_next().as_deref(), Some("Cube3x3 0"));
        wait_for_count(&count, SCRAMBLES_AHEAD + 1);
    }

    #[test]
    fn switching_back_reuses_what_was_prepared() {
        let (queue, count) = counting_queue(WcaEvent::Cube3x3);
        wait_for_count(&count, SCRAMBLES_AHEAD);

        queue.switch_event(WcaEvent::Cube2x2, None);
        wait_for_count(&count, 2 * SCRAMBLES_AHEAD);
        assert_eq!(queue.try_next().as_deref(), Some("Cube2x2 2"));
        wait_for_count(&count, 2 * SCRAMBLES_AHEAD + 1);

        queue.switch_event(WcaEvent::Cube3x3, None);
        assert_eq!(queue.try_next().as_deref(), Some("Cube3x3 0"));
    }

    #[test]
    fn an_unused_scramble_comes_back_first() {
        let (queue, count) = counting_queue(WcaEvent::Cube3x3);
        wait_for_count(&count, SCRAMBLES_AHEAD);
        let shown = queue.try_next();
        wait_for_count(&count, SCRAMBLES_AHEAD + 1);

        queue.switch_event(WcaEvent::Cube2x2, shown.clone());
        queue.switch_event(WcaEvent::Cube3x3, None);

        assert_eq!(queue.try_next(), shown);
    }

    #[test]
    fn a_slow_solve_neither_delays_the_next_event_nor_is_lost() {
        let (started, wait_for_start) = mpsc::channel();
        let (finish, wait_for_finish) = mpsc::channel::<()>();
        let wait_for_finish = Mutex::new(wait_for_finish);
        let queue = ScrambleQueue::with_generator(WcaEvent::Fto, move |event| {
            if event == WcaEvent::Fto {
                started.send(()).ok();
                wait_for_finish.lock().unwrap().recv().ok();
            }
            format!("{event:?}")
        });
        wait_for_start.recv().unwrap();

        queue.switch_event(WcaEvent::Skewb, None);
        assert_eq!(eventually(|| queue.try_next()), "Skewb");

        finish.send(()).unwrap();
        queue.switch_event(WcaEvent::Fto, None);
        assert_eq!(eventually(|| queue.try_next()), "Fto");
    }

    #[test]
    fn discarding_drops_prepared_scrambles_and_the_running_solve() {
        let (started, wait_for_start) = mpsc::channel();
        let (finish, wait_for_finish) = mpsc::channel::<()>();
        let wait_for_finish = Mutex::new(wait_for_finish);
        let count = AtomicUsize::new(0);
        let queue = ScrambleQueue::with_generator(WcaEvent::Cube3x3, move |_| {
            let number = count.fetch_add(1, Ordering::SeqCst) + 1;
            if number == 2 {
                started.send(()).ok();
                wait_for_finish.lock().unwrap().recv().ok();
            }
            number.to_string()
        });
        wait_for_start.recv().unwrap();

        queue.discard(WcaEvent::Cube3x3);
        finish.send(()).unwrap();

        let scramble = eventually(|| queue.try_next());
        assert_eq!(scramble, "3");
    }

    #[test]
    fn the_signal_fires_when_a_scramble_arrives() {
        let (queue, count) = counting_queue(WcaEvent::Cube3x3);
        let mut signal = queue.signal();

        assert!(block_on(signal.changed()));
        assert!(queue.try_next().is_some());

        // Even a delivery from before the wait began is reported.
        wait_for_count(&count, SCRAMBLES_AHEAD + 1);
        assert!(block_on(signal.changed()));
    }

    #[test]
    fn dropping_the_queue_stops_the_worker_and_releases_the_signal() {
        let (alive, stopped) = mpsc::channel::<()>();
        let queue = ScrambleQueue::with_generator(WcaEvent::Cube3x3, move |_| {
            alive.send(()).ok();
            String::new()
        });
        let mut signal = queue.signal();
        assert!(block_on(signal.changed()));

        let waiter = thread::spawn(move || while block_on(signal.changed()) {});
        drop(queue);

        waiter.join().unwrap();
        // The worker shares the sender, so the channel ends when the worker does.
        while stopped.recv_timeout(PATIENCE).is_ok() {}
        assert_eq!(stopped.try_recv(), Err(mpsc::TryRecvError::Disconnected));
    }
}
