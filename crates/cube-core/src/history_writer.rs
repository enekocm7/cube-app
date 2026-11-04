//! Saves the solve history on a background thread.
//!
//! Serializing a large history takes long enough to stall the UI right as the
//! next attempt can begin, so the UI only hands over a snapshot. Writes happen
//! in submission order, and a snapshot that is replaced before the thread gets
//! to it is skipped, since the newer one contains everything the older had.
//!
//! The thread owns the [`HistoryStore`], so merging what cube-tui wrote happens
//! here too and the result travels back as a [`Merge`] the UI adopts when it is
//! safe to. The UI may record more solves first: a merge it can no longer adopt
//! is dropped, and its next snapshot is merged again with the file, which still
//! has everything, so nothing depends on the UI adopting any one merge.

use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use crate::history::History;
use crate::merge::MergeSummary;
use crate::persistence::HistoryStore;

/// A merge finished in the background, waiting for the UI.
pub struct Merge {
    sessions: Vec<History>,
    summary: MergeSummary,
    /// The store's merge count, once this merge is included.
    merges: u64,
    /// The snapshot this merge is based on.
    generation: u64,
}

/// Something the writer has to tell the UI.
pub enum WriterEvent {
    Merged(Merge),
    SaveFailed(String),
    BackupFailed(String),
    /// A backup asked for with [`HistoryWriter::back_up`], and where it went.
    BackedUp(PathBuf),
}

/// A snapshot to save.
struct Request {
    sessions: Vec<History>,
    generation: u64,
    /// How many merges this snapshot already contains.
    seen_merges: u64,
    /// Whether to skip the write when the file has not changed.
    refresh: bool,
}

#[derive(Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent flags for queued save, backup folder, backup and shutdown"
)]
struct State {
    /// The newest snapshot not yet written.
    pending: Option<Request>,
    /// A new folder for the user's backups, not yet passed to the store.
    /// `None` turns them off. `user_backups_dirty` tells an update apart
    /// from no update, without nesting `Option`.
    user_backups: Option<PathBuf>,
    user_backups_dirty: bool,
    /// Whether a backup was asked for.
    back_up: bool,
    /// Whether the thread is working right now.
    writing: bool,
    /// What the caller has not been told yet.
    events: Vec<WriterEvent>,
    closing: bool,
    /// The newest snapshot handed over.
    generation: u64,
    /// How many merges the caller has adopted.
    seen_merges: u64,
}

impl State {
    const fn has_work(&self) -> bool {
        self.pending.is_some() || self.user_backups_dirty || self.back_up
    }
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
    /// Starts a writer that saves through `store`.
    pub fn new(store: HistoryStore) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            work: Condvar::new(),
            idle: Condvar::new(),
        });
        let worker = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("history-writer".to_owned())
            .spawn(move || run(&worker, store))
            .ok();
        Self { shared, thread }
    }

    /// Queues `sessions` to be saved, replacing any snapshot still waiting.
    pub fn save(&self, sessions: Vec<History>) {
        let mut state = lock(&self.shared.state);
        state.generation += 1;
        let request = Request {
            sessions,
            generation: state.generation,
            seen_merges: state.seen_merges,
            refresh: false,
        };
        state.pending = Some(request);
        self.shared.work.notify_one();
    }

    /// Queues a check for what another app wrote, with `sessions` as the
    /// current history. A save already waiting does that check itself.
    pub fn refresh(&self, sessions: Vec<History>) {
        let mut state = lock(&self.shared.state);
        if state.pending.is_none() {
            let request = Request {
                sessions,
                generation: state.generation,
                seen_merges: state.seen_merges,
                refresh: true,
            };
            state.pending = Some(request);
            self.shared.work.notify_one();
        }
    }

    /// Takes a merge in, returning the sessions to show and what changed.
    ///
    /// Returns `None` when a newer snapshot was handed over since, in which
    /// case the merge no longer has everything and its own merge will follow.
    pub fn adopt(&self, merge: Merge) -> Option<(Vec<History>, MergeSummary)> {
        let mut state = lock(&self.shared.state);
        if merge.generation != state.generation {
            return None;
        }
        state.seen_merges = merge.merges;
        // Holds what was just replaced, so it would only repeat this merge.
        if state
            .pending
            .as_ref()
            .is_some_and(|pending| pending.refresh)
        {
            state.pending = None;
        }
        Some((merge.sessions, merge.summary))
    }

    /// Sets the folder for the user's own backups; `None` turns them off.
    pub fn set_user_backups(&self, dir: Option<PathBuf>) {
        let mut state = lock(&self.shared.state);
        state.user_backups = dir;
        state.user_backups_dirty = true;
        self.shared.work.notify_one();
    }

    /// Queues a backup to the user's folder, after anything already queued.
    pub fn back_up(&self) {
        lock(&self.shared.state).back_up = true;
        self.shared.work.notify_one();
    }

    /// Takes what happened since the last call, oldest first.
    pub fn take_events(&self) -> Vec<WriterEvent> {
        std::mem::take(&mut lock(&self.shared.state).events)
    }

    /// Blocks until everything queued so far has been written.
    pub fn flush(&self) {
        let mut state = lock(&self.shared.state);
        while state.has_work() || state.writing {
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

/// Saves `request`, returning what the caller should hear about it.
fn save(store: &mut HistoryStore, request: Request) -> Option<WriterEvent> {
    let Request {
        mut sessions,
        generation,
        seen_merges,
        refresh,
    } = request;
    let saved = if refresh {
        store.refresh(&mut sessions, seen_merges)
    } else {
        store.save(&mut sessions, seen_merges)
    };
    match saved {
        Ok(Some(summary)) => Some(WriterEvent::Merged(Merge {
            sessions,
            summary,
            merges: store.merges(),
            generation,
        })),
        Ok(None) => None,
        Err(error) => Some(WriterEvent::SaveFailed(format!("{error:#}"))),
    }
}

fn run(shared: &Shared, mut store: HistoryStore) {
    loop {
        let (request, user_backups, back_up) = {
            let mut state = lock(&shared.state);
            loop {
                if state.has_work() {
                    state.writing = true;
                    let user_backups = if state.user_backups_dirty {
                        state.user_backups_dirty = false;
                        Some(state.user_backups.take())
                    } else {
                        None
                    };
                    break (
                        state.pending.take(),
                        user_backups,
                        std::mem::take(&mut state.back_up),
                    );
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

        let mut events = Vec::new();
        if let Some(dir) = user_backups {
            store.set_user_backups(dir);
        }
        if let Some(request) = request {
            events.extend(save(&mut store, request));
        }
        if let Some(error) = store.take_backup_error() {
            events.push(WriterEvent::BackupFailed(format!("{error:#}")));
        }
        if back_up {
            events.push(match store.back_up_now() {
                Ok(path) => WriterEvent::BackedUp(path),
                Err(error) => WriterEvent::BackupFailed(format!("{error:#}")),
            });
        }

        let mut state = lock(&shared.state);
        state.writing = false;
        state.events.append(&mut events);
        shared.idle.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::event::WcaEvent;
    use crate::solve::{Modifier, Solve};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    fn temporary_history() -> PathBuf {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("cube-writer-{}-{id}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        dir.join("times.json")
    }

    fn remove_folder_of(path: &Path) {
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    fn writer_at(path: &Path) -> HistoryWriter {
        HistoryWriter::new(HistoryStore::open(path).unwrap().0)
    }

    fn load(path: &Path) -> Vec<History> {
        HistoryStore::open(path).unwrap().1
    }

    fn named(name: &str) -> Vec<History> {
        let mut history = History::new();
        history.set_name(name.to_owned());
        vec![history]
    }

    fn solve(ms: u64) -> Solve {
        Solve::new_with_meta(
            ms,
            WcaEvent::Cube3x3,
            format!("R{ms}"),
            ms,
            Modifier::None,
            String::new(),
        )
    }

    fn session(times: &[u64]) -> Vec<History> {
        let mut history = History::new();
        for &ms in times {
            history.push(solve(ms));
        }
        vec![history]
    }

    fn times(sessions: &[History]) -> Vec<u64> {
        sessions[0].solves().iter().map(Solve::raw_ms).collect()
    }

    /// Writes `sessions` the way cube-tui does: straight over the file.
    fn write_like_cube_tui(path: &Path, sessions: &[History]) {
        fs::write(path, serde_json::to_string_pretty(sessions).unwrap()).unwrap();
    }

    /// Waits for the writer and returns the merge it finished last.
    fn finished_merge(writer: &HistoryWriter) -> Merge {
        writer.flush();
        writer
            .take_events()
            .into_iter()
            .rev()
            .find_map(|event| match event {
                WriterEvent::Merged(merge) => Some(merge),
                _ => None,
            })
            .expect("no merge happened")
    }

    #[test]
    fn flush_waits_for_the_last_snapshot() {
        let path = temporary_history();
        let writer = writer_at(&path);
        for name in ["a", "b", "c"] {
            writer.save(named(name));
        }
        writer.flush();
        assert_eq!(load(&path)[0].name(), "c");
        assert_eq!(writer.take_events().len(), 0);
        remove_folder_of(&path);
    }

    #[test]
    fn dropping_flushes() {
        let path = temporary_history();
        let writer = writer_at(&path);
        writer.save(named("last"));
        drop(writer);
        assert_eq!(load(&path)[0].name(), "last");
        remove_folder_of(&path);
    }

    #[test]
    fn failures_are_reported_once() {
        let path = temporary_history();
        let writer = writer_at(&path);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
        writer.save(named("x"));
        writer.flush();
        let events = writer.take_events();
        assert!(matches!(events[..], [WriterEvent::SaveFailed(_)]));
        assert_eq!(writer.take_events().len(), 0);
    }

    #[test]
    fn a_merge_reaches_the_ui() {
        let path = temporary_history();
        write_like_cube_tui(&path, &session(&[10]));
        let writer = writer_at(&path);
        write_like_cube_tui(&path, &session(&[10, 20]));

        writer.save(session(&[10, 30]));
        let (sessions, summary) = writer.adopt(finished_merge(&writer)).unwrap();

        assert_eq!(times(&sessions), [10, 20, 30]);
        assert_eq!(summary.added, 1);
        assert_eq!(times(&load(&path)), [10, 20, 30]);
        remove_folder_of(&path);
    }

    #[test]
    fn a_refresh_finds_outside_changes_without_a_new_solve() {
        let path = temporary_history();
        write_like_cube_tui(&path, &session(&[10]));
        let writer = writer_at(&path);
        writer.refresh(session(&[10]));
        writer.flush();
        assert_eq!(writer.take_events().len(), 0);

        write_like_cube_tui(&path, &session(&[10, 20]));
        writer.refresh(session(&[10]));
        let (sessions, _) = writer.adopt(finished_merge(&writer)).unwrap();
        assert_eq!(times(&sessions), [10, 20]);
        remove_folder_of(&path);
    }

    #[test]
    fn a_solve_recorded_before_adopting_a_merge_loses_nothing() {
        let path = temporary_history();
        write_like_cube_tui(&path, &session(&[10]));
        let writer = writer_at(&path);
        write_like_cube_tui(&path, &session(&[10, 20]));
        writer.save(session(&[10, 30]));
        let stale = finished_merge(&writer);

        // The UI moved on without seeing solve 20.
        writer.save(session(&[10, 30, 40]));
        let merge = finished_merge(&writer);

        assert!(writer.adopt(stale).is_none());
        let (sessions, _) = writer.adopt(merge).unwrap();
        assert_eq!(times(&sessions), [10, 20, 30, 40]);
        assert_eq!(times(&load(&path)), [10, 20, 30, 40]);
        remove_folder_of(&path);
    }

    #[test]
    fn a_deletion_made_before_adopting_a_merge_is_honoured() {
        let path = temporary_history();
        write_like_cube_tui(&path, &session(&[10, 15]));
        let writer = writer_at(&path);
        write_like_cube_tui(&path, &session(&[10, 15, 20]));
        writer.save(session(&[10, 15, 30]));
        let _ = finished_merge(&writer);

        writer.save(session(&[10, 30]));
        let (sessions, _) = writer.adopt(finished_merge(&writer)).unwrap();

        assert_eq!(times(&sessions), [10, 20, 30]);
        assert_eq!(times(&load(&path)), [10, 20, 30]);
        remove_folder_of(&path);
    }

    #[test]
    fn a_merged_solve_deleted_after_adopting_stays_deleted() {
        let path = temporary_history();
        write_like_cube_tui(&path, &session(&[10]));
        let writer = writer_at(&path);
        write_like_cube_tui(&path, &session(&[10, 20]));
        writer.save(session(&[10]));
        assert!(writer.adopt(finished_merge(&writer)).is_some());

        writer.save(session(&[10]));
        writer.flush();

        assert_eq!(writer.take_events().len(), 0);
        assert_eq!(times(&load(&path)), [10]);
        remove_folder_of(&path);
    }

    #[test]
    fn backups_run_after_what_is_queued() {
        let path = temporary_history();
        let folder = path.with_file_name("mine");
        let writer = writer_at(&path);
        writer.set_user_backups(Some(folder));
        writer.save(session(&[10]));
        writer.back_up();
        writer.flush();

        let events = writer.take_events();
        let [WriterEvent::BackedUp(backup)] = &events[..] else {
            panic!("the backup did not finish");
        };
        assert_eq!(fs::read(backup).unwrap(), fs::read(&path).unwrap());
        remove_folder_of(&path);
    }

    #[test]
    fn a_failed_backup_is_reported() {
        let path = temporary_history();
        let writer = writer_at(&path);
        writer.back_up();
        writer.flush();
        let events = writer.take_events();
        assert!(matches!(events[..], [WriterEvent::BackupFailed(_)]));
        remove_folder_of(&path);
    }
}
