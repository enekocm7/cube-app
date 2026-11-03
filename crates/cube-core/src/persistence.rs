//! Reading and writing the data directory.
//!
//! The solve history is cube-tui's `times.json`, in cube-tui's directory, so one
//! save works in both apps. Like cube-tui, debug builds use a `debug` subfolder
//! and never touch real data; `CUBE_DATA_DIR` overrides the location entirely.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{BufReader, ErrorKind};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::history::History;
use crate::settings::Settings;

/// Returns the shared data directory, creating it when needed.
pub fn data_dir() -> Result<PathBuf> {
    let dir = match std::env::var_os("CUBE_DATA_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => {
            let dir = dirs::data_dir()
                .context("Could not determine the application data directory")?
                .join("cube-tui");
            if cfg!(debug_assertions) {
                dir.join("debug")
            } else {
                dir
            }
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

/// Loads the sessions at `path`; `None` when no history exists yet.
///
/// A file that exists but cannot be read is an error, never an empty history:
/// treating it as empty would let the next save destroy it.
pub fn load_history(path: &Path) -> Result<Option<Vec<History>>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("Could not read {}", path.display()));
        }
    };
    serde_json::from_reader(BufReader::new(file))
        .map(Some)
        .with_context(|| format!("Invalid solve history in {}", path.display()))
}

/// Saves `sessions` to `path` atomically.
pub fn save_history(path: &Path, sessions: &[History]) -> Result<()> {
    let json = serde_json::to_string_pretty(sessions).context("Could not serialize solves")?;
    write_atomic(path, json.as_bytes())
        .with_context(|| format!("Could not save solves to {}", path.display()))
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

/// Writes through a sibling temporary file so a crash cannot truncate `path`.
fn write_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let mut temporary = OsString::from(path.as_os_str());
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    fs::write(&temporary, contents)?;
    fs::rename(&temporary, path)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::event::WcaEvent;
    use crate::solve::Modifier;

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

    #[test]
    fn history_survives_a_save_and_load() {
        let path = temporary_path();
        let sessions: Vec<History> = serde_json::from_str(CUBE_TUI_SAVE).unwrap();
        save_history(&path, &sessions).unwrap();
        let loaded = load_history(&path).unwrap().unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].solves()[0].raw_ms(), 12_345);
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_missing_history_differs_from_a_corrupt_one() {
        let path = temporary_path();
        assert!(load_history(&path).unwrap().is_none());
        fs::write(&path, "not json").unwrap();
        assert!(load_history(&path).is_err());
        fs::remove_file(&path).unwrap();
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
