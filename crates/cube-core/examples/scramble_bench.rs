//! Prints per-event scramble timings as the tables in
//! `website/src/content/docs/scramble-performance.md`.
//!
//! ```sh
//! cargo run --release -p cube-core --example scramble_bench
//! ```

use std::sync::Arc;
use std::task::{Context, Poll, Wake};
use std::thread;
use std::time::{Duration, Instant};

use cube_core::event::WcaEvent;
use cube_core::scramble::{self, ScrambleQueue};

const SOLVES: u32 = 20;
/// Long enough for the worker to refill the queue on the slowest event.
const SETTLE: Duration = Duration::from_millis(1500);

struct Unpark(thread::Thread);

impl Wake for Unpark {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    let waker = Arc::new(Unpark(thread::current())).into();
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
        thread::park();
    }
}

fn timed<T>(work: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let value = work();
    (value, start.elapsed())
}

fn millis(duration: Duration) -> String {
    format!("{:.2} ms", duration.as_secs_f64() * 1000.0)
}

/// Prints the first call, which builds tables, and the steady-state mean.
fn solver_row(label: &str, event: WcaEvent) {
    let (_, first) = timed(|| scramble::generate(event));
    let ((), total) = timed(|| {
        for _ in 0..SOLVES {
            scramble::generate(event);
        }
    });
    println!(
        "| {label} | {} | {} |",
        millis(first),
        millis(total / SOLVES)
    );
}

/// Prints how long taking a scramble takes when one is prepared, and when the
/// queue is empty and the wait goes through `ScrambleSignal`.
fn queue_row(event: WcaEvent) {
    let queue = ScrambleQueue::new(event);
    let mut signal = queue.signal();
    thread::sleep(SETTLE);

    let (scramble, prepared) = timed(|| queue.try_next());
    assert!(scramble.is_some(), "{event:?} was not ready in time");

    while queue.try_next().is_some() {}
    let (_, empty) = timed(|| {
        loop {
            if let Some(scramble) = queue.try_next() {
                return scramble;
            }
            block_on(signal.changed());
        }
    });
    println!("| {event:?} | {} | {} |", millis(prepared), millis(empty));
}

fn main() {
    println!("| event | first call (incl. tables) | steady state |");
    println!("|---|---|---|");
    for event in WcaEvent::ALL {
        solver_row(&format!("{event:?}"), event);
    }
    scramble::set_accurate_3x3(true);
    solver_row("Cube3x3 (shorter scrambles)", WcaEvent::Cube3x3);
    scramble::set_accurate_3x3(false);

    println!();
    println!("| event | scramble prepared | queue empty |");
    println!("|---|---|---|");
    for event in WcaEvent::ALL {
        queue_row(event);
    }
}
