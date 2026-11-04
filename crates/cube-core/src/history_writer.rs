//! Saves the solve history on a background thread.
//!
//! Serializing a large history takes long enough to stall the UI right as the
//! next attempt can begin, so the UI only hands over a snapshot. Writes happen
//! in submission order, and a snapshot that is replaced before the thread gets
//! to it is skipped, since the newer one contains everything the older had.

use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use crate::history::History;
use crate::persistence;

#[derive(Default)]
struct State {
    /// The newest snapshot not yet written.
    pending: Option<Vec<History>>,
    /// Whether the thread is writing a snapshot right now.
    writing: bool,
    /// Failures not yet reported to the caller.
    errors: Vec<String>,
    closing: bool,
}

struct Shared {
    state: Mutex<State>,
    /// Wakes the thread when work arrives or the writer closes.
    work: Condvar,
    /// Wakes `flush` when the thread goes idle.
    idle: Condvar,
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Writes history snapshots to one file in the background.
///
/// Dropping the writer flushes: it waits for the last snapshot to reach disk.
pub struct HistoryWriter {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl HistoryWriter {
    /// Starts a writer that saves to `path`.
    pub fn new(path: PathBuf) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            work: Condvar::new(),
            idle: Condvar::new(),
        });
        let worker = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("history-writer".to_owned())
            .spawn(move || run(&worker, &path))
            .ok();
        Self { shared, thread }
    }

    /// Queues `sessions` to be saved, replacing any snapshot still waiting.
    pub fn save(&self, sessions: Vec<History>) {
        let mut state = lock(&self.shared.state);
        state.pending = Some(sessions);
        self.shared.work.notify_one();
    }

    /// Takes the failures since the last call, oldest first.
    pub fn take_errors(&self) -> Vec<String> {
        std::mem::take(&mut lock(&self.shared.state).errors)
    }

    /// Blocks until everything queued so far has been written.
    pub fn flush(&self) {
        let mut state = lock(&self.shared.state);
        while state.pending.is_some() || state.writing {
            state = self
                .shared
                .idle
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}

impl Drop for HistoryWriter {
    fn drop(&mut self) {
        lock(&self.shared.state).closing = true;
        self.shared.work.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run(shared: &Shared, path: &std::path::Path) {
    loop {
        let sessions = {
            let mut state = lock(&shared.state);
            loop {
                if let Some(sessions) = state.pending.take() {
                    state.writing = true;
                    break sessions;
                }
                if state.closing {
                    return;
                }
                shared.idle.notify_all();
                state = shared
                    .work
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
            }
        };
        let result = persistence::save_history(path, &sessions);
        let mut state = lock(&shared.state);
        state.writing = false;
        if let Err(error) = result {
            state.errors.push(format!("{error:#}"));
        }
        shared.idle.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    fn temporary_path() -> PathBuf {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("cube-writer-{}-{id}.json", std::process::id()))
    }

    fn named(name: &str) -> Vec<History> {
        let mut history = History::new();
        history.set_name(name.to_owned());
        vec![history]
    }

    #[test]
    fn flush_waits_for_the_last_snapshot() {
        let path = temporary_path();
        let writer = HistoryWriter::new(path.clone());
        for name in ["a", "b", "c"] {
            writer.save(named(name));
        }
        writer.flush();
        let saved = persistence::load_history(&path).unwrap().unwrap();
        assert_eq!(saved[0].name(), "c");
        assert_eq!(writer.take_errors().len(), 0);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn dropping_flushes() {
        let path = temporary_path();
        let writer = HistoryWriter::new(path.clone());
        writer.save(named("last"));
        drop(writer);
        let saved = persistence::load_history(&path).unwrap().unwrap();
        assert_eq!(saved[0].name(), "last");
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn failures_are_reported_once() {
        let path = std::env::temp_dir()
            .join("cube-writer-missing-dir")
            .join("times.json");
        let writer = HistoryWriter::new(path);
        writer.save(named("x"));
        writer.flush();
        assert_eq!(writer.take_errors().len(), 1);
        assert_eq!(writer.take_errors().len(), 0);
    }
}
