//! Reading and writing csTimer's JSON export.
//!
//! csTimer stores each session as `sessionN`, an array of solves written as
//! `[[penalty_ms, time_ms], scramble, comment, timestamp]`. cube-tui imports
//! and exports the same format, so a file moved between the two apps keeps its
//! solves. csTimer records no event, so the scramble decides it.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};

use crate::event::classify_event;
use crate::history::History;
use crate::persistence;
use crate::solve::{Modifier, Solve};

/// The result of importing a csTimer export.
#[derive(Debug)]
pub struct ImportReport {
    /// The imported sessions, in csTimer's session order.
    pub sessions: Vec<History>,
    /// How many solves were imported.
    pub imported: usize,
    /// How many solves were skipped because they could not be read.
    pub skipped: usize,
}

/// Imports a csTimer JSON export.
///
/// Only `sessionN` arrays are read; other properties are ignored. Solves that
/// are not in csTimer's shape are counted as skipped instead of failing the
/// whole import, so one bad entry cannot cost the rest of the file.
pub fn import(path: &Path) -> Result<ImportReport> {
    let file = File::open(path).with_context(|| format!("Could not read {}", path.display()))?;
    let root: Value = serde_json::from_reader(BufReader::new(file))
        .with_context(|| format!("Invalid csTimer export in {}", path.display()))?;
    let Value::Object(root) = root else {
        bail!("{} is not a csTimer export", path.display());
    };

    let mut report = ImportReport {
        sessions: Vec::new(),
        imported: 0,
        skipped: 0,
    };
    for value in ordered_sessions(&root) {
        let mut history = History::new();
        let Value::Array(solves) = value else {
            report.skipped += 1;
            continue;
        };
        for solve in solves {
            match parse_solve(solve) {
                Some(solve) => {
                    history.push(solve);
                    report.imported += 1;
                }
                None => report.skipped += 1,
            }
        }
        report.sessions.push(history);
    }
    Ok(report)
}

/// Writes `sessions` as a csTimer JSON export, returning the file it wrote.
///
/// Like cube-tui, the path gains a `.json` extension unless it is already a
/// `.txt`, which csTimer also accepts.
pub fn export(path: &Path, sessions: &[History]) -> Result<PathBuf> {
    let mut root = Map::new();
    for (index, history) in sessions.iter().enumerate() {
        let solves: Vec<Value> = history.solves().iter().map(solve_value).collect();
        root.insert(format!("session{}", index + 1), Value::Array(solves));
    }
    let json = serde_json::to_string_pretty(&Value::Object(root))
        .context("Could not serialize the csTimer export")?;
    let path = export_path(path);
    persistence::write_atomic(&path, json.as_bytes())
        .with_context(|| format!("Could not write {}", path.display()))?;
    Ok(path)
}

/// Returns the csTimer session values ordered by their numeric index.
fn ordered_sessions(root: &Map<String, Value>) -> impl Iterator<Item = &Value> {
    let mut sessions: BTreeMap<usize, &Value> = BTreeMap::new();
    for (key, value) in root {
        if let Some(index) = key
            .strip_prefix("session")
            .and_then(|suffix| suffix.parse().ok())
        {
            sessions.insert(index, value);
        }
    }
    sessions.into_values()
}

/// Reads one solve from csTimer's positional array representation.
fn parse_solve(value: &Value) -> Option<Solve> {
    let Value::Array(fields) = value else {
        return None;
    };
    let [
        Value::Array(penalty_and_time),
        scramble,
        comment,
        timestamp,
        ..,
    ] = fields.as_slice()
    else {
        return None;
    };
    let [Value::Number(penalty), Value::Number(time)] = penalty_and_time.as_slice() else {
        return None;
    };
    let penalty = penalty.as_i64()?;
    let time_ms = time.as_u64()?;
    let scramble = scramble.as_str()?.to_owned();
    let comment = comment.as_str().unwrap_or_default().to_owned();
    let timestamp = timestamp.as_u64()?;
    let modifier = match penalty {
        -1 => Modifier::DNF,
        penalty if penalty > 0 => Modifier::PlusTwo,
        _ => Modifier::None,
    };
    Some(Solve::new_with_meta(
        time_ms,
        classify_event(&scramble),
        scramble,
        normalize_timestamp_ms(timestamp),
        modifier,
        comment,
    ))
}

/// Writes one solve in csTimer's positional array representation.
fn solve_value(solve: &Solve) -> Value {
    let penalty = match solve.modifier() {
        Modifier::None => 0,
        Modifier::PlusTwo => 2_000,
        Modifier::DNF => -1,
    };
    json!([
        [penalty, solve.raw_ms()],
        solve.scramble(),
        solve.comment(),
        normalize_timestamp_seconds(solve.solved_at_unix_ms()),
    ])
}

/// Returns the path to write, adding the extension csTimer expects.
fn export_path(path: &Path) -> PathBuf {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("txt") => path.to_path_buf(),
        _ => path.with_extension("json"),
    }
}

/// Converts legacy second timestamps to the millisecond representation used
/// internally.
const fn normalize_timestamp_ms(timestamp: u64) -> u64 {
    if timestamp < 1_000_000_000_000 {
        timestamp.saturating_mul(1000)
    } else {
        timestamp
    }
}

/// Converts millisecond timestamps to the seconds expected in csTimer exports.
const fn normalize_timestamp_seconds(timestamp_ms: u64) -> u64 {
    if timestamp_ms >= 1_000_000_000_000 {
        timestamp_ms / 1000
    } else {
        timestamp_ms
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::event::WcaEvent;

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    fn temporary_path(extension: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "cube-core-cstimer-{}-{}.{extension}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn import_json(json: &str) -> ImportReport {
        let path = temporary_path("json");
        std::fs::write(&path, json).unwrap();
        let report = import(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        report
    }

    #[test]
    fn imports_sessions_in_index_order_and_ignores_other_keys() {
        let report = import_json(
            r#"{
                "session3": [[[0, 3000], "R U", "", 1700000000]],
                "session1": [[[0, 1000], "R U", "", 1700000100]],
                "session10": [[[0, 10000], "R U", "", 1700000200]],
                "session2": [[[0, 2000], "R U", "", 1700000300]],
                "properties": {"sessionData": "{}"}
            }"#,
        );
        let times: Vec<u64> = report
            .sessions
            .iter()
            .map(|history| history.solves()[0].raw_ms())
            .collect();
        assert_eq!(times, [1000, 2000, 3000, 10000]);
        assert_eq!(report.imported, 4);
        assert_eq!(report.skipped, 0);
    }

    #[test]
    fn imports_penalties_timestamps_events_and_comments() {
        let report = import_json(
            r#"{
                "session1": [
                    [[0, 1234], "R U R' U' F2 D L B' R2 F' U2 B2 L' D' F R U", "nice", 1700000000],
                    [[2000, 2000], "R U R' U' F2 D L B' R2 F' U2 B2 L' D' F R U", "", 1700000001],
                    [[-1, 3000], "(1,0) / (-1,-1) /", "", 1700000002]
                ]
            }"#,
        );
        let solves = report.sessions[0].solves();
        assert_eq!(solves[0].raw_ms(), 1234);
        assert_eq!(solves[0].modifier(), Modifier::None);
        assert_eq!(solves[0].solved_at_unix_ms(), 1_700_000_000_000);
        assert_eq!(solves[0].comment(), "nice");
        assert_eq!(solves[0].event(), WcaEvent::Cube3x3);
        assert_eq!(solves[1].modifier(), Modifier::PlusTwo);
        assert_eq!(solves[2].modifier(), Modifier::DNF);
        assert_eq!(solves[2].event(), WcaEvent::Square1);
        assert_eq!(report.imported, 3);
    }

    #[test]
    fn counts_solves_it_cannot_read_as_skipped() {
        let report = import_json(
            r#"{
                "session1": [
                    [[0, 1000], "R U", "", 1700000000],
                    [[0, -5], "R U", "", 1700000000],
                    ["not a solve"],
                    [[0, 2000]]
                ],
                "notASession": [[[0, 9999], "R U", "", 1700000000]]
            }"#,
        );
        assert_eq!(report.imported, 1);
        assert_eq!(report.skipped, 3);
        assert_eq!(report.sessions.len(), 1);
    }

    #[test]
    fn an_empty_export_has_no_sessions() {
        let report = import_json(r#"{"properties": {"sessionData": "{}"}}"#);
        assert!(report.sessions.is_empty());
        assert_eq!(report.imported, 0);
        assert_eq!(report.skipped, 0);
    }

    #[test]
    fn a_file_that_is_not_an_object_is_an_error() {
        let path = temporary_path("json");
        std::fs::write(&path, "[]").unwrap();
        assert!(import(&path).is_err());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn exports_what_it_imports() {
        let report = import_json(
            r#"{
                "session1": [
                    [[0, 1234], "R U R' U' F2 D L B' R2 F' U2 B2 L' D' F R U", "nice", 1700000000],
                    [[2000, 2000], "R U R' U' F2 D L B' R2 F' U2 B2 L' D' F R U", "", 1700000001],
                    [[-1, 3000], "(1,0) / (-1,-1) /", "", 1700000002]
                ]
            }"#,
        );
        let path = temporary_path("json");
        let written = export(&path, &report.sessions).unwrap();
        let restored = import(&written).unwrap();
        std::fs::remove_file(&written).unwrap();

        let before = report.sessions[0].solves();
        let after = restored.sessions[0].solves();
        assert_eq!(before.len(), after.len());
        for (before, after) in before.iter().zip(after) {
            assert_eq!(before.raw_ms(), after.raw_ms());
            assert_eq!(before.modifier(), after.modifier());
            assert_eq!(before.scramble(), after.scramble());
            assert_eq!(before.comment(), after.comment());
            assert_eq!(before.event(), after.event());
            assert_eq!(before.solved_at_unix_ms(), after.solved_at_unix_ms());
        }
    }

    #[test]
    fn exports_every_session_with_a_json_extension() {
        let mut first = History::new();
        first.push(Solve::new(
            10_000,
            WcaEvent::Cube3x3,
            "R U".into(),
            Modifier::None,
        ));
        let path = temporary_path("backup");
        let written = export(&path, &[first]).unwrap();
        assert_eq!(written.extension().unwrap(), "json");
        let restored = import(&written).unwrap();
        std::fs::remove_file(&written).unwrap();
        assert_eq!(restored.sessions.len(), 1);
        assert_eq!(restored.sessions[0].len(), 1);
    }
}
