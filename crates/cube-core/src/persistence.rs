//! Reading and writing the data directory.
//!
//! The solve history is cube-tui's `times.json`, in cube-tui's directory, so one
//! save works in both apps. Like cube-tui, debug builds use a `debug` subfolder
//! and never touch real data; `CUBE_DATA_DIR` overrides the location entirely.
//!
//! cube-tui can rewrite `times.json` at any moment, so saving goes through
//! [`HistoryStore`], which merges those changes in instead of overwriting them
//! and keeps timed copies of the file in a `backups` folder, and in a folder of
//! the user's own when they choose one.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, TimeDelta, Utc};

use crate::history::History;
use crate::merge::{self, MergeSummary};
use crate::settings::Settings;
use crate::theme::{self, ThemeColors, ThemeFile};

/// Returns the shared data directory, creating it when needed.
pub fn data_dir() -> Result<PathBuf> {
    let dir = if let Some(dir) = std::env::var_os("CUBE_DATA_DIR") {
        PathBuf::from(dir)
    } else {
        let dir = dirs::data_dir()
            .context("Could not determine the application data directory")?
            .join("cube-tui");
        if cfg!(debug_assertions) {
            dir.join("debug")
        } else {
            dir
        }
    };
    fs::create_dir_all(&dir).with_context(|| format!("Could not create {}", dir.display()))?;
    Ok(dir)
}

/// Returns the solve history shared with cube-tui.
pub fn history_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("times.json"))
}

/// Returns the settings file of the desktop app.
pub fn settings_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("cube-gui.toml"))
}

/// Returns the folder holding cube-tui theme files, creating it when needed.
pub fn themes_dir() -> Result<PathBuf> {
    let dir = data_dir()?.join("themes");
    fs::create_dir_all(&dir).with_context(|| format!("Could not create {}", dir.display()))?;
    Ok(dir)
}

/// Loads a theme by file name, adding `.toml` when it is missing.
pub fn load_theme(name: &str) -> Result<ThemeColors> {
    theme::load_named(&themes_dir()?, name)
}

/// Lists every valid theme in the shared themes folder.
pub fn list_themes() -> Result<Vec<ThemeFile>> {
    theme::list(&themes_dir()?)
}

/// Creates cube-tui's default theme the first time the themes folder is made.
///
/// An existing folder is left alone, even when it is empty, so a theme the
/// user deleted does not come back.
pub fn ensure_default_theme() -> Result<()> {
    let dir = data_dir()?.join("themes");
    if dir.try_exists()? {
        return Ok(());
    }
    fs::create_dir_all(&dir).with_context(|| format!("Could not create {}", dir.display()))?;
    theme::write_default(&dir.join("default.toml"))
}

/// How many times an operation is tried when another program may be in the way.
const ATTEMPTS: u32 = 5;
/// The wait between attempts; the UI thread waits this, so it stays short.
const RETRY_DELAY: Duration = Duration::from_millis(50);
/// The least time between two backups, so a busy session does not rotate older
/// backups away.
const BACKUP_INTERVAL: TimeDelta = TimeDelta::hours(6);
/// How many backups are kept.
const BACKUP_LIMIT: usize = 12;
/// The `chrono` format of the UTC time in a backup's name.
const BACKUP_TIME_FORMAT: &str = "%Y%m%dT%H%M%SZ";
/// How old a leftover temporary file must be to count as abandoned.
const STALE_TEMPORARY_AGE: Duration = Duration::from_hours(1);

/// The history file as read just now.
struct CurrentFile {
    /// `None` when the file does not exist.
    bytes: Option<Vec<u8>>,
    /// The file's sessions, when they were asked for or someone else changed it.
    theirs: Option<Vec<History>>,
}

/// The solve history on disk, shared with cube-tui.
pub struct HistoryStore {
    path: PathBuf,
    /// The file's contents when this app last read or wrote it, to tell when
    /// someone else changed it and as the base of a merge.
    synced: Option<Vec<u8>>,
    /// The folder the user chose for backups, on top of the app's own.
    user_backups: Option<PathBuf>,
    /// Whether the last backup to `user_backups` failed, so a missing drive is
    /// reported once and not on every solve.
    user_backups_failing: bool,
    /// A backup failure the user has not been told about.
    backup_error: Option<anyhow::Error>,
    /// How many merges have happened. The caller may not have taken in the
    /// latest yet, so each save says how many of them its sessions contain.
    merges: u64,
    /// The sessions as handed to the latest merge, before it. For a caller who
    /// has not taken that merge in, they are the last version both sides share.
    merge_base: Option<Vec<u8>>,
}

impl HistoryStore {
    /// Opens the history at `path`, returning its sessions, which are empty
    /// when none exists yet.
    ///
    /// A file that exists but cannot be read is an error, never an empty
    /// history: treating it as empty would let the next save destroy it.
    pub fn open(path: &Path) -> Result<(Self, Vec<History>)> {
        remove_stale_temporaries(path);
        let synced =
            read_if_present(path).with_context(|| format!("Could not read {}", path.display()))?;
        let sessions = synced
            .as_deref()
            .map(|bytes| parse_history(bytes, path))
            .transpose()?
            .unwrap_or_default();
        if let Some(bytes) = &synced {
            // A failed backup must not stop the app; saving reports real
            // problems with the folder.
            let _ = back_up_if_due(&backups_dir(path), bytes, Utc::now());
        }
        let store = Self {
            path: path.to_owned(),
            synced,
            user_backups: None,
            user_backups_failing: false,
            backup_error: None,
            merges: 0,
            merge_base: None,
        };
        Ok((store, sessions))
    }

    /// Returns how many merges have happened, to compare with what a caller
    /// has taken in.
    pub const fn merges(&self) -> u64 {
        self.merges
    }

    /// Saves `sessions`, first merging in anything another app wrote.
    ///
    /// `seen_merges` is how many merges `sessions` contains. When it is behind,
    /// the file has solves `sessions` never had, not solves it deleted, so they
    /// are merged in as well.
    ///
    /// Returns what a merge did, in which case `sessions` changed. When the
    /// file cannot be read it is left alone, and `sessions` go to a sibling
    /// file so they survive the app closing.
    pub fn save(
        &mut self,
        sessions: &mut Vec<History>,
        seen_merges: u64,
    ) -> Result<Option<MergeSummary>> {
        let behind = seen_merges != self.merges;
        let current = match self.read_current(behind) {
            Ok(current) => current,
            Err(error) => return Err(self.keep_unsaved(sessions, error)),
        };
        let theirs = match current.theirs {
            // The file went missing, but what it last held still counts.
            None if behind => Some(self.parse_known(self.synced.as_deref())?),
            theirs => theirs,
        };
        let summary = match theirs {
            Some(theirs) => {
                let base = if behind {
                    self.merge_base.as_deref()
                } else {
                    self.synced.as_deref()
                };
                let base = self.parse_known(base)?;
                let line = serde_json::to_vec(sessions).context("Could not serialize solves")?;
                self.merge_base = Some(line);
                self.merges += 1;
                Some(merge::merge(&base, sessions, &theirs))
            }
            None => None,
        };

        let json = serde_json::to_vec_pretty(sessions).context("Could not serialize solves")?;
        if current.bytes.as_ref() != Some(&json) {
            if let Some(bytes) = &current.bytes {
                self.back_up(bytes);
            }
            write_atomic(&self.path, &json)
                .with_context(|| format!("Could not save solves to {}", self.path.display()))?;
        }
        self.synced = Some(json);
        Ok(summary)
    }

    /// Merges in anything another app wrote, saving the result. Does nothing
    /// when the file is as this app left it and `sessions` has seen every merge.
    pub fn refresh(
        &mut self,
        sessions: &mut Vec<History>,
        seen_merges: u64,
    ) -> Result<Option<MergeSummary>> {
        match read_if_present(&self.path) {
            Ok(current) if current == self.synced && seen_merges == self.merges => Ok(None),
            _ => self.save(sessions, seen_merges),
        }
    }

    /// Parses a history this store read or wrote itself.
    fn parse_known(&self, bytes: Option<&[u8]>) -> Result<Vec<History>> {
        bytes.map_or_else(|| Ok(Vec::new()), |bytes| parse_history(bytes, &self.path))
    }

    /// Sets the folder for the user's own backups; `None` turns them off.
    pub fn set_user_backups(&mut self, dir: Option<PathBuf>) {
        if self.user_backups != dir {
            self.user_backups = dir;
            self.user_backups_failing = false;
        }
    }

    /// Returns a backup failure the user has not been told about yet.
    pub const fn take_backup_error(&mut self) -> Option<anyhow::Error> {
        self.backup_error.take()
    }

    /// Copies the history into the user's folder right away. These copies are
    /// the user's to manage, so rotation never removes them.
    pub fn back_up_now(&self) -> Result<PathBuf> {
        let dir = self
            .user_backups
            .as_deref()
            .context("No backup folder is set")?;
        let contents = self
            .synced
            .as_deref()
            .context("There are no solves to back up yet")?;
        write_manual_backup(dir, contents, Utc::now())
            .with_context(|| format!("Could not write to {}", dir.display()))
    }

    /// Makes the backups that are due. Neither one failing stops the save.
    fn back_up(&mut self, contents: &[u8]) {
        let now = Utc::now();
        // Saving reports real problems with the app's own folder.
        let _ = back_up_if_due(&backups_dir(&self.path), contents, now);
        let Some(dir) = &self.user_backups else {
            return;
        };
        match back_up_if_due(dir, contents, now) {
            Ok(()) => self.user_backups_failing = false,
            Err(error) if !self.user_backups_failing => {
                self.user_backups_failing = true;
                let context = format!("Could not write to {}", dir.display());
                self.backup_error = Some(anyhow::Error::new(error).context(context));
            }
            Err(_) => {}
        }
    }

    /// Reads the file, also parsing it when someone else changed it or when
    /// `always_parse` asks for it.
    ///
    /// cube-tui does not write atomically, so a file that fails to parse may
    /// just be half written and is given a moment to finish.
    fn read_current(&self, always_parse: bool) -> Result<CurrentFile> {
        let mut attempt = 1;
        loop {
            let read = read_if_present(&self.path)
                .with_context(|| format!("Could not read {}", self.path.display()))
                .and_then(|bytes| {
                    let theirs = match &bytes {
                        Some(bytes) if always_parse || self.synced.as_ref() != Some(bytes) => {
                            Some(parse_history(bytes, &self.path)?)
                        }
                        _ => None,
                    };
                    Ok(CurrentFile { bytes, theirs })
                });
            if read.is_ok() || attempt == ATTEMPTS {
                return read;
            }
            attempt += 1;
            thread::sleep(RETRY_DELAY);
        }
    }

    /// Writes `sessions` beside the unreadable history and describes where.
    fn keep_unsaved(&self, sessions: &[History], error: anyhow::Error) -> anyhow::Error {
        let path = self.path.with_extension("unsaved.json");
        let written = serde_json::to_vec_pretty(sessions)
            .map_err(io::Error::from)
            .and_then(|json| write_atomic(&path, &json));
        match written {
            Ok(()) => error.context(format!("Your solves were kept in {}", path.display())),
            Err(_) => error,
        }
    }
}

fn parse_history(bytes: &[u8], path: &Path) -> Result<Vec<History>> {
    serde_json::from_slice(bytes)
        .with_context(|| format!("Invalid solve history in {}", path.display()))
}

/// Reads `path`; `None` when it does not exist.
fn read_if_present(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Returns the folder of backups for the history at `path`.
fn backups_dir(path: &Path) -> PathBuf {
    path.with_file_name("backups")
}

/// Lists the backups in `dir`, oldest first. Other files are ignored.
fn list_backups(dir: &Path) -> io::Result<Vec<(DateTime<Utc>, PathBuf)>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut backups = Vec::new();
    for entry in entries {
        let path = entry?.path();
        let time = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix("times-")?.strip_suffix(".json"))
            .and_then(|time| NaiveDateTime::parse_from_str(time, BACKUP_TIME_FORMAT).ok());
        if let Some(time) = time {
            backups.push((time.and_utc(), path));
        }
    }
    backups.sort();
    Ok(backups)
}

/// Copies `contents` into `dir` unless a recent backup exists, then drops the
/// oldest backups past the limit. Only ever given a history that parsed, so a
/// damaged file cannot push good backups out.
fn back_up_if_due(dir: &Path, contents: &[u8], now: DateTime<Utc>) -> io::Result<()> {
    let mut backups = list_backups(dir)?;
    if let Some((newest, _)) = backups.last() {
        let age = now - *newest;
        if age >= TimeDelta::zero() && age < BACKUP_INTERVAL {
            return Ok(());
        }
    }
    fs::create_dir_all(dir)?;
    let path = dir.join(format!("times-{}.json", now.format(BACKUP_TIME_FORMAT)));
    write_atomic(&path, contents)?;
    backups.push((now, path));
    backups.sort();
    let excess = backups.len().saturating_sub(BACKUP_LIMIT);
    for (_, path) in &backups[..excess] {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// Writes a backup the user asked for. Its name keeps it out of `list_backups`,
/// and with that out of rotation.
fn write_manual_backup(dir: &Path, contents: &[u8], now: DateTime<Utc>) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let name = format!("times-manual-{}.json", now.format(BACKUP_TIME_FORMAT));
    let path = dir.join(name);
    write_atomic(&path, contents)?;
    Ok(path)
}

/// Returns the newest backup of the history at `path`, to point the user to
/// when the history cannot be read.
pub fn latest_backup(path: &Path) -> Option<PathBuf> {
    let (_, newest) = list_backups(&backups_dir(path)).ok()?.pop()?;
    Some(newest)
}

/// Loads settings, falling back to the defaults when there are none yet.
pub fn load_settings(path: &Path) -> Result<Settings> {
    match fs::read_to_string(path) {
        Ok(text) => {
            toml::from_str(&text).with_context(|| format!("Invalid settings in {}", path.display()))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Settings::default()),
        Err(error) => Err(error).with_context(|| format!("Could not read {}", path.display())),
    }
}

/// Saves settings to `path` atomically.
pub fn save_settings(path: &Path, settings: &Settings) -> Result<()> {
    let text = toml::to_string_pretty(settings).context("Could not serialize settings")?;
    write_atomic(path, text.as_bytes())
        .with_context(|| format!("Could not save settings to {}", path.display()))
}

/// Moves a file that could not be read out of the way, so saving over it
/// destroys nothing, and returns its new path.
pub fn set_aside(path: &Path) -> io::Result<PathBuf> {
    let mut aside = OsString::from(path.as_os_str());
    aside.push(format!(
        ".invalid-{}",
        Utc::now().format(BACKUP_TIME_FORMAT)
    ));
    let aside = PathBuf::from(aside);
    fs::rename(path, &aside)?;
    Ok(aside)
}

/// Writes through a sibling temporary file, so a crash or power cut leaves
/// either the old contents or the new, never a mix.
pub(crate) fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    let temporary = temporary_sibling(path);
    let written = write_synced(&temporary, contents).and_then(|()| replace(&temporary, path));
    if written.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    written?;
    sync_parent(path);
    Ok(())
}

/// Returns a temporary name no other write is using, even in another process.
fn temporary_sibling(path: &Path) -> PathBuf {
    static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);
    let mut temporary = OsString::from(path.as_os_str());
    temporary.push(format!(
        ".{}-{}.tmp",
        std::process::id(),
        NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
    ));
    PathBuf::from(temporary)
}

/// Writes `path` and waits for the data to reach the disk, so the rename that
/// follows cannot land first and leave an empty file after a power cut.
fn write_synced(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(contents)?;
    file.sync_all()
}

/// Renames `from` over `to`. Windows refuses while anything has `to` open, such
/// as cube-tui reading it or an antivirus scan, so this waits a moment for that.
fn replace(from: &Path, to: &Path) -> io::Result<()> {
    let mut attempt = 1;
    loop {
        match fs::rename(from, to) {
            Err(error) if error.kind() != ErrorKind::NotFound && attempt < ATTEMPTS => {
                attempt += 1;
                thread::sleep(RETRY_DELAY);
            }
            result => return result,
        }
    }
}

/// Makes a rename inside the parent folder durable. Windows has no equivalent,
/// and the file is already safe by now, so a failure here changes nothing.
fn sync_parent(path: &Path) {
    if cfg!(unix)
        && let Some(parent) = path.parent()
        && let Ok(parent) = File::open(parent)
    {
        let _ = parent.sync_all();
    }
}

/// Deletes temporary files a crash left beside `path`. Recent ones are left
/// alone, since they may belong to a write still in progress.
fn remove_stale_temporaries(path: &Path) {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return;
    };
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    let prefix = format!("{}.", name.to_string_lossy());
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let abandoned = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age >= STALE_TEMPORARY_AGE);
        if abandoned && file_name.starts_with(&prefix) && file_name.ends_with(".tmp") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::WcaEvent;
    use crate::solve::{Modifier, Solve};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    /// A file as written by cube-tui, including a legacy unnamed session.
    const CUBE_TUI_SAVE: &str = r#"[
      {
        "session_name": "Practice",
        "times": [
          {
            "timestamp_in_millis": 12345,
            "event": "Cube3x3",
            "scramble": "R U R' U'",
            "solved_at_unix_ms": 1760000000000,
            "modifier": "PlusTwo",
            "comment": "nice",
            "changes": [
              {
                "changed_at_unix_ms": 1760000000001,
                "before": {
                  "time_ms": 12345, "event": "Cube3x3", "scramble": "R U R' U'",
                  "solved_at_unix_ms": 1760000000000, "modifier": "None", "comment": ""
                },
                "after": {
                  "time_ms": 12345, "event": "Cube3x3", "scramble": "R U R' U'",
                  "solved_at_unix_ms": 1760000000000, "modifier": "PlusTwo", "comment": "nice"
                }
              }
            ],
            "was_single_record": true
          }
        ]
      },
      { "times": [ { "timestamp_in_millis": 1, "event": "Square1", "scramble": "" } ] }
    ]"#;

    fn temporary_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "cube-core-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn reads_files_written_by_cube_tui() {
        let sessions: Vec<History> = serde_json::from_str(CUBE_TUI_SAVE).unwrap();
        assert_eq!(sessions[0].name(), "Practice");
        let solve = &sessions[0].solves()[0];
        assert_eq!(solve.raw_ms(), 12_345);
        assert_eq!(solve.modifier(), Modifier::PlusTwo);
        assert_eq!(solve.comment(), "nice");
        assert_eq!(solve.changes().len(), 1);
        assert!(solve.was_single_record());
        assert_eq!(sessions[1].solves()[0].event(), WcaEvent::Square1);
        assert_eq!(sessions[1].solves()[0].solved_at_unix_ms(), 0);
    }

    #[test]
    fn writes_the_same_shape_it_reads() {
        let sessions: Vec<History> = serde_json::from_str(CUBE_TUI_SAVE).unwrap();
        let written = serde_json::to_value(&sessions).unwrap();
        let session = &written[0];
        assert_eq!(session["session_name"], "Practice");
        assert_eq!(session["times"][0]["timestamp_in_millis"], 12_345);
        assert_eq!(session["times"][0]["modifier"], "PlusTwo");
        assert!(written[1].get("session_name").is_none());
        assert!(written[1]["times"][0].get("comment").is_none());
        assert!(written[1]["times"][0].get("changes").is_none());
    }

    /// Returns where a history goes inside a new folder of its own.
    fn temporary_history() -> PathBuf {
        let dir = temporary_path();
        fs::create_dir(&dir).unwrap();
        dir.join("times.json")
    }

    fn remove_folder_of(path: &Path) {
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
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

    fn session(times: &[u64]) -> History {
        let mut history = History::new();
        for &ms in times {
            history.push(solve(ms));
        }
        history
    }

    fn times(sessions: &[History]) -> Vec<u64> {
        sessions[0].solves().iter().map(Solve::raw_ms).collect()
    }

    /// Writes `sessions` the way cube-tui does: straight over the file.
    fn write_like_cube_tui(path: &Path, sessions: &[History]) {
        fs::write(path, serde_json::to_string_pretty(sessions).unwrap()).unwrap();
    }

    fn at(hours: i64) -> DateTime<Utc> {
        DateTime::UNIX_EPOCH + TimeDelta::hours(hours)
    }

    #[test]
    fn history_survives_a_save_and_load() {
        let path = temporary_history();
        let (mut store, mut sessions) = HistoryStore::open(&path).unwrap();
        assert!(sessions.is_empty());
        sessions = serde_json::from_str(CUBE_TUI_SAVE).unwrap();
        assert!(store.save(&mut sessions, store.merges()).unwrap().is_none());
        let (_, loaded) = HistoryStore::open(&path).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].solves()[0].raw_ms(), 12_345);
        remove_folder_of(&path);
    }

    #[test]
    fn a_missing_history_differs_from_a_corrupt_one() {
        let path = temporary_history();
        assert!(HistoryStore::open(&path).unwrap().1.is_empty());
        fs::write(&path, "not json").unwrap();
        assert!(HistoryStore::open(&path).is_err());
        remove_folder_of(&path);
    }

    #[test]
    fn saving_merges_what_cube_tui_wrote_instead_of_dropping_it() {
        let path = temporary_history();
        write_like_cube_tui(&path, &[session(&[10])]);
        let (mut store, mut sessions) = HistoryStore::open(&path).unwrap();

        write_like_cube_tui(&path, &[session(&[10, 20])]);
        sessions[0].push(solve(30));
        let summary = store.save(&mut sessions, store.merges()).unwrap().unwrap();

        assert_eq!(summary.added, 1);
        assert_eq!(times(&sessions), [10, 20, 30]);
        let (_, on_disk) = HistoryStore::open(&path).unwrap();
        assert_eq!(times(&on_disk), [10, 20, 30]);
        remove_folder_of(&path);
    }

    #[test]
    fn refresh_only_acts_on_outside_changes() {
        let path = temporary_history();
        let (mut store, _) = HistoryStore::open(&path).unwrap();
        let mut sessions = vec![session(&[10])];
        store.save(&mut sessions, store.merges()).unwrap();
        assert!(
            store
                .refresh(&mut sessions, store.merges())
                .unwrap()
                .is_none()
        );

        write_like_cube_tui(&path, &[session(&[10, 20])]);
        let summary = store
            .refresh(&mut sessions, store.merges())
            .unwrap()
            .unwrap();
        assert_eq!(summary.added, 1);
        assert_eq!(times(&sessions), [10, 20]);
        assert!(
            store
                .refresh(&mut sessions, store.merges())
                .unwrap()
                .is_none()
        );
        remove_folder_of(&path);
    }

    #[test]
    fn a_deleted_file_is_written_again() {
        let path = temporary_history();
        let (mut store, _) = HistoryStore::open(&path).unwrap();
        let mut sessions = vec![session(&[10])];
        store.save(&mut sessions, store.merges()).unwrap();
        fs::remove_file(&path).unwrap();
        assert!(
            store
                .refresh(&mut sessions, store.merges())
                .unwrap()
                .is_none()
        );
        assert_eq!(times(&HistoryStore::open(&path).unwrap().1), [10]);
        remove_folder_of(&path);
    }

    #[test]
    fn a_damaged_file_is_not_overwritten_and_solves_are_kept_beside_it() {
        let path = temporary_history();
        let (mut store, _) = HistoryStore::open(&path).unwrap();
        let mut sessions = vec![session(&[10])];
        store.save(&mut sessions, store.merges()).unwrap();

        fs::write(&path, "[{ half written").unwrap();
        let error = store.save(&mut sessions, store.merges()).unwrap_err();

        let unsaved = path.with_extension("unsaved.json");
        assert!(format!("{error:#}").contains("unsaved.json"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "[{ half written");
        let kept: Vec<History> = serde_json::from_slice(&fs::read(unsaved).unwrap()).unwrap();
        assert_eq!(times(&kept), [10]);
        remove_folder_of(&path);
    }

    #[test]
    fn opening_backs_up_the_file_as_it_was() {
        let path = temporary_history();
        write_like_cube_tui(&path, &[session(&[10])]);
        let original = fs::read(&path).unwrap();
        let (mut store, mut sessions) = HistoryStore::open(&path).unwrap();
        sessions[0].push(solve(20));
        store.save(&mut sessions, store.merges()).unwrap();

        let backup = latest_backup(&path).unwrap();
        assert_eq!(fs::read(backup).unwrap(), original);
        remove_folder_of(&path);
    }

    #[test]
    fn backups_are_spaced_out_and_limited() {
        let dir = temporary_path();
        back_up_if_due(&dir, b"first", at(0)).unwrap();
        back_up_if_due(&dir, b"too soon", at(5)).unwrap();
        assert_eq!(list_backups(&dir).unwrap().len(), 1);

        for step in 1..=20 {
            back_up_if_due(&dir, b"later", at(step * 6)).unwrap();
        }
        let backups = list_backups(&dir).unwrap();
        assert_eq!(backups.len(), BACKUP_LIMIT);
        assert_eq!(backups.last().unwrap().0, at(120));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_backup_dated_in_the_future_does_not_stop_new_ones() {
        let dir = temporary_path();
        back_up_if_due(&dir, b"clock was ahead", at(100)).unwrap();
        back_up_if_due(&dir, b"clock fixed", at(1)).unwrap();
        assert_eq!(list_backups(&dir).unwrap().len(), 2);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn other_files_in_the_backups_folder_are_left_alone() {
        let dir = temporary_path();
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("notes.txt"), "mine").unwrap();
        fs::write(dir.join("times-yesterday.json"), "mine").unwrap();
        assert_eq!(list_backups(&dir).unwrap().len(), 0);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_chosen_folder_gets_backups_on_top_of_the_apps_own() {
        let path = temporary_history();
        let folder = path.with_file_name("mine");
        write_like_cube_tui(&path, &[session(&[10])]);
        let original = fs::read(&path).unwrap();
        let (mut store, mut sessions) = HistoryStore::open(&path).unwrap();
        store.set_user_backups(Some(folder.clone()));
        sessions[0].push(solve(20));
        store.save(&mut sessions, store.merges()).unwrap();

        let backups = list_backups(&folder).unwrap();
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::read(&backups[0].1).unwrap(), original);
        assert!(latest_backup(&path).is_some());
        remove_folder_of(&path);
    }

    #[test]
    fn an_unusable_folder_is_reported_once_and_does_not_stop_saving() {
        let path = temporary_history();
        let not_a_folder = path.with_file_name("file");
        fs::write(&not_a_folder, "").unwrap();
        write_like_cube_tui(&path, &[session(&[10])]);
        let (mut store, mut sessions) = HistoryStore::open(&path).unwrap();
        store.set_user_backups(Some(not_a_folder));

        sessions[0].push(solve(20));
        store.save(&mut sessions, store.merges()).unwrap();
        assert!(store.take_backup_error().is_some());
        assert_eq!(times(&HistoryStore::open(&path).unwrap().1), [10, 20]);

        sessions[0].push(solve(30));
        store.save(&mut sessions, store.merges()).unwrap();
        assert!(store.take_backup_error().is_none());
        remove_folder_of(&path);
    }

    #[test]
    fn manual_backups_copy_the_history_and_are_never_rotated() {
        let path = temporary_history();
        let folder = path.with_file_name("mine");
        let (mut store, _) = HistoryStore::open(&path).unwrap();
        assert!(store.back_up_now().is_err());
        store.set_user_backups(Some(folder.clone()));
        assert!(store.back_up_now().is_err());

        let mut sessions = vec![session(&[10])];
        store.save(&mut sessions, store.merges()).unwrap();
        let manual = store.back_up_now().unwrap();
        assert_eq!(fs::read(&manual).unwrap(), fs::read(&path).unwrap());

        for step in 0..=13 {
            back_up_if_due(&folder, b"later", at(step * 6)).unwrap();
        }
        assert!(manual.exists());
        remove_folder_of(&path);
    }

    #[test]
    fn atomic_writes_replace_the_file_and_leave_nothing_behind() {
        let path = temporary_history();
        write_atomic(&path, b"old").unwrap();
        write_atomic(&path, b"new").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        let files = fs::read_dir(path.parent().unwrap()).unwrap().count();
        assert_eq!(files, 1);
        remove_folder_of(&path);
    }

    #[test]
    fn a_failed_atomic_write_leaves_no_temporary_behind() {
        let path = temporary_history();
        // A file cannot be renamed over a folder.
        let folder = path.with_file_name("folder");
        fs::create_dir(&folder).unwrap();
        assert!(write_atomic(&folder, b"new").is_err());
        assert!(folder.is_dir());
        let files = fs::read_dir(path.parent().unwrap()).unwrap().count();
        assert_eq!(files, 1);
        remove_folder_of(&path);
    }

    #[test]
    fn only_old_temporaries_are_cleaned_up() {
        let path = temporary_history();
        let recent = path.with_file_name("times.json.1-0.tmp");
        let old = path.with_file_name("times.json.tmp");
        let unrelated = path.with_file_name("other.tmp");
        for file in [&recent, &old, &unrelated] {
            fs::write(file, "partial").unwrap();
        }
        let long_ago = SystemTime::now() - STALE_TEMPORARY_AGE * 2;
        for file in [&old, &unrelated] {
            File::options()
                .write(true)
                .open(file)
                .unwrap()
                .set_modified(long_ago)
                .unwrap();
        }
        remove_stale_temporaries(&path);
        assert!(recent.exists());
        assert!(!old.exists());
        assert!(unrelated.exists());
        remove_folder_of(&path);
    }

    #[test]
    fn set_aside_keeps_the_contents_under_a_new_name() {
        let path = temporary_history();
        fs::write(&path, "[broken").unwrap();
        let aside = set_aside(&path).unwrap();
        assert!(!path.exists());
        assert_eq!(fs::read_to_string(aside).unwrap(), "[broken");
        remove_folder_of(&path);
    }

    #[test]
    fn settings_default_when_missing_and_fail_when_corrupt() {
        let path = temporary_path();
        assert_eq!(load_settings(&path).unwrap(), Settings::default());
        fs::write(&path, "[broken").unwrap();
        assert!(load_settings(&path).is_err());
        fs::remove_file(&path).unwrap();
    }
}
