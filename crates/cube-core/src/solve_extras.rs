//! `solve-extras.json`: what this app knows about a solve that `times.json`
//! cannot hold.
//!
//! cube-tui's solve has no catch-all for unknown fields, so anything added to
//! `times.json` would be dropped the next time it saves. Extras therefore live
//! in a file of their own, keyed by [`SolveId`], which survives edits.
//!
//! In memory the extras ride on the [`Solve`](crate::solve::Solve) itself, so
//! they follow it through edits, deletions, undo and merges. The file is
//! worked out again from every snapshot saved. A solve that goes missing from
//! `times.json` keeps its entry for the same reason a merge never lets the
//! file remove a solve, and the entry comes back if the solve does. An entry
//! only goes away when this app deletes the solve.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::history::History;
use crate::merge::{self, SolveId};
use crate::persistence::{read_if_present, write_atomic};
use crate::solve::Solve;
use crate::tags::{self, TagChange};

/// The name of the file, beside `times.json`.
const FILE_NAME: &str = "solve-extras.json";

/// One solve's extras.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    #[serde(flatten)]
    id: SolveId,
    #[serde(flatten)]
    extras: Extras,
}

/// What one solve has beyond `times.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Extras {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    solution: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    memo_ms: Option<u64>,
}

impl Extras {
    fn of(solve: &Solve) -> Self {
        Self {
            tags: solve.tags().to_vec(),
            solution: solve.solution().to_owned(),
            memo_ms: solve.memo_ms(),
        }
    }

    fn is_empty(&self) -> bool {
        self.tags.is_empty() && self.solution.is_empty() && self.memo_ms.is_none()
    }
}

/// The contents of the file.
#[derive(Default, Serialize, Deserialize)]
struct ExtrasFile {
    solves: Vec<Entry>,
}

/// Returns each solve with an id that is unique in the whole history.
fn identified(sessions: &[History]) -> impl Iterator<Item = (SolveId, &Solve)> {
    let mut seen = HashMap::new();
    sessions
        .iter()
        .flat_map(History::solves)
        .map(move |solve| (merge::identify(solve, &mut seen), solve))
}

/// Parses the file, refusing anything the app could not save back unchanged.
fn parse(bytes: &[u8], path: &Path) -> Result<ExtrasFile> {
    let file: ExtrasFile = serde_json::from_slice(bytes)
        .with_context(|| format!("Invalid solve extras in {}", path.display()))?;
    let valid = |tag: &&String| tags::parse(tag).is_ok_and(|parsed| parsed == **tag);
    let mut every_tag = file.solves.iter().flat_map(|entry| &entry.extras.tags);
    match every_tag.find(|tag| !valid(tag)) {
        Some(tag) => Err(anyhow!("Invalid tag {tag:?} in {}", path.display())),
        None => Ok(file),
    }
}

/// The extras on disk, beside the history they belong to.
pub(crate) struct SolveExtras {
    path: PathBuf,
    /// Entries of solves the history does not have.
    missing: HashMap<SolveId, Extras>,
    /// The file's contents when this app last read or wrote it.
    synced: Option<Vec<u8>>,
}

impl SolveExtras {
    /// Opens the extras of the history at `history`, handing them to
    /// `sessions`.
    ///
    /// A file that exists but cannot be read is an error, never empty extras:
    /// treating it as empty would let the next save destroy it.
    pub(crate) fn open(history: &Path, sessions: &mut [History]) -> Result<Self> {
        let path = history.with_file_name(FILE_NAME);
        let synced =
            read_if_present(&path).with_context(|| format!("Could not read {}", path.display()))?;
        let file = synced
            .as_deref()
            .map(|bytes| parse(bytes, &path))
            .transpose()?
            .unwrap_or_default();
        let missing = file
            .solves
            .into_iter()
            .map(|entry| (entry.id, entry.extras))
            .collect();
        let mut extras = Self {
            path,
            missing,
            synced,
        };
        extras.hand_over(sessions);
        let present: HashSet<SolveId> = identified(sessions).map(|(id, _)| id).collect();
        extras.missing.retain(|id, _| !present.contains(id));
        Ok(extras)
    }

    /// Lets go of the entries of solves in `sessions`, which carry their own
    /// extras from now on. Call this before merging, while `sessions` is only
    /// what the caller knows about: a solve the caller has not taken in yet
    /// still needs its entry for the next merge.
    pub(crate) fn release_present(&mut self, sessions: &[History]) {
        if !self.missing.is_empty() {
            for (id, _) in identified(sessions) {
                self.missing.remove(&id);
            }
        }
    }

    /// Gives the solves that came back their extras.
    pub(crate) fn hand_over(&self, sessions: &mut [History]) {
        if self.missing.is_empty() {
            return;
        }
        let ids: Vec<SolveId> = identified(sessions).map(|(id, _)| id).collect();
        let solves = sessions.iter_mut().flat_map(History::solves_mut);
        for (id, solve) in ids.iter().zip(solves) {
            if let Some(extras) = self.missing.get(id) {
                solve.set_tags(extras.tags.clone());
                solve.set_solution(extras.solution.clone());
                solve.set_memo_ms(extras.memo_ms);
            }
        }
    }

    /// Applies a change to the entries no solve is carrying.
    pub(crate) fn change_tags(&mut self, change: &TagChange) {
        for extras in self.missing.values_mut() {
            change.apply(&mut extras.tags);
        }
        self.missing.retain(|_, extras| !extras.is_empty());
    }

    /// Writes the extras of `sessions`, when the file would change.
    pub(crate) fn save(&mut self, sessions: &[History]) -> Result<()> {
        let mut solves: Vec<Entry> = identified(sessions)
            .map(|(id, solve)| Entry {
                id,
                extras: Extras::of(solve),
            })
            .filter(|entry| !entry.extras.is_empty())
            .collect();
        // A solve here carries its own extras, even when it has none left.
        let present: HashSet<SolveId> = if self.missing.is_empty() {
            HashSet::new()
        } else {
            identified(sessions).map(|(id, _)| id).collect()
        };
        let mut missing: Vec<Entry> = self
            .missing
            .iter()
            .filter(|(id, _)| !present.contains(*id))
            .map(|(id, extras)| Entry {
                id: id.clone(),
                extras: extras.clone(),
            })
            .collect();
        // The map has no order of its own, and the file should only change
        // when its contents do.
        missing.sort_by(|first, second| first.id.cmp(&second.id));
        solves.append(&mut missing);

        // No file and nothing to put in it stays no file.
        if solves.is_empty() && self.synced.is_none() {
            return Ok(());
        }
        let json = serde_json::to_vec_pretty(&ExtrasFile { solves })
            .context("Could not serialize solve extras")?;
        if self.synced.as_ref() != Some(&json) {
            write_atomic(&self.path, &json)
                .with_context(|| format!("Could not save to {}", self.path.display()))?;
            self.synced = Some(json);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::event::WcaEvent;
    use crate::solve::{Modifier, SolveSnapshot};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    /// A `times.json` that does not exist yet, in a new folder.
    fn temporary_history() -> PathBuf {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("cube-extras-{}-{id}", std::process::id()));
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

    fn session(times: &[u64]) -> Vec<History> {
        let mut history = History::new();
        for &ms in times {
            history.push(solve(ms));
        }
        vec![history]
    }

    fn tag(sessions: &mut [History], index: usize, tag: &str) {
        assert!(sessions[0].set_tags(index, vec![tag.to_owned()]));
    }

    fn tags_of(sessions: &[History]) -> Vec<Vec<String>> {
        let solves = sessions[0].solves().iter();
        solves.map(|solve| solve.tags().to_vec()).collect()
    }

    fn saved(path: &Path, sessions: &[History]) -> SolveExtras {
        let mut extras = SolveExtras::open(path, &mut session(&[])).unwrap();
        extras.save(sessions).unwrap();
        extras
    }

    #[test]
    fn nothing_tagged_writes_no_file() {
        let path = temporary_history();
        saved(&path, &session(&[10]));
        assert!(!path.with_file_name(FILE_NAME).exists());
        remove_folder_of(&path);
    }

    #[test]
    fn tags_come_back_on_the_solves_they_were_saved_on() {
        let path = temporary_history();
        let mut sessions = session(&[10, 20]);
        tag(&mut sessions, 1, "GAN");
        saved(&path, &sessions);

        let mut loaded = session(&[10, 20]);
        SolveExtras::open(&path, &mut loaded).unwrap();
        assert_eq!(tags_of(&loaded), [vec![], vec!["GAN".to_owned()]]);
        remove_folder_of(&path);
    }

    #[test]
    fn solutions_come_back_and_leave_the_tags_alone() {
        let path = temporary_history();
        let mut sessions = session(&[10, 20]);
        tag(&mut sessions, 0, "GAN");
        assert!(sessions[0].set_solution(1, "R U // cross".to_owned()));
        assert!(!sessions[0].set_solution(1, "R U // cross".to_owned()));
        saved(&path, &sessions);

        let mut loaded = session(&[10, 20]);
        SolveExtras::open(&path, &mut loaded).unwrap();
        assert_eq!(tags_of(&loaded), [vec!["GAN".to_owned()], vec![]]);
        assert_eq!(loaded[0].solves()[0].solution(), "");
        assert_eq!(loaded[0].solves()[1].solution(), "R U // cross");
        remove_folder_of(&path);
    }

    #[test]
    fn a_memo_comes_back_and_never_reaches_times_json() {
        let path = temporary_history();
        let mut sessions = session(&[10, 20]);
        sessions[0].solves_mut()[1].set_memo_ms(Some(7));
        saved(&path, &sessions);

        let json = serde_json::to_string(&sessions).unwrap();
        assert!(!json.contains("memo"));

        let mut loaded = session(&[10, 20]);
        SolveExtras::open(&path, &mut loaded).unwrap();
        let memos: Vec<_> = loaded[0].solves().iter().map(Solve::memo_ms).collect();
        assert_eq!(memos, [None, Some(7)]);
        assert_eq!(loaded[0].solves()[1].execution_ms(), Some(13));
        remove_folder_of(&path);
    }

    #[test]
    fn a_memo_survives_a_solve_going_missing_and_an_edit() {
        let path = temporary_history();
        let mut sessions = session(&[10, 20]);
        sessions[0].solves_mut()[1].set_memo_ms(Some(7));
        saved(&path, &sessions);

        // Missing from the history, the entry stays and comes back.
        let mut without = session(&[10]);
        let mut extras = SolveExtras::open(&path, &mut without).unwrap();
        extras.save(&without).unwrap();
        let mut back = session(&[10, 20]);
        extras.hand_over(&mut back);
        assert_eq!(back[0].solves()[1].memo_ms(), Some(7));

        // An edit keeps the memo and shortens it with the time.
        let after = SolveSnapshot {
            time_ms: 5,
            ..sessions[0].solves()[1].snapshot()
        };
        assert!(sessions[0].edit(1, after));
        assert_eq!(sessions[0].solves()[1].memo_ms(), Some(5));
        assert_eq!(sessions[0].solves()[1].execution_ms(), Some(0));
        remove_folder_of(&path);
    }

    #[test]
    fn a_solution_never_reaches_times_json() {
        let mut sessions = session(&[10]);
        assert!(sessions[0].set_solution(0, "R U".to_owned()));
        let json = serde_json::to_string(&sessions).unwrap();
        assert!(!json.contains("solution"));
    }

    #[test]
    fn a_solve_that_goes_missing_keeps_its_solution() {
        let path = temporary_history();
        let mut sessions = session(&[10]);
        assert!(sessions[0].set_solution(0, "R U".to_owned()));
        saved(&path, &sessions);

        let mut extras = SolveExtras::open(&path, &mut session(&[])).unwrap();
        // Tidying tags on what is left must not take the solution along.
        extras.change_tags(&TagChange::Remove {
            tag: "GAN".to_owned(),
        });
        extras.save(&session(&[])).unwrap();

        let mut back = session(&[10]);
        SolveExtras::open(&path, &mut back).unwrap();
        assert_eq!(back[0].solves()[0].solution(), "R U");
        remove_folder_of(&path);
    }

    #[test]
    fn a_file_without_solutions_stays_that_way() {
        let path = temporary_history();
        let mut sessions = session(&[10]);
        tag(&mut sessions, 0, "GAN");
        saved(&path, &sessions);
        let file = fs::read_to_string(path.with_file_name(FILE_NAME)).unwrap();
        assert!(!file.contains("solution"));
        remove_folder_of(&path);
    }

    #[test]
    fn tags_survive_an_edit_to_the_solve() {
        let path = temporary_history();
        let mut sessions = session(&[10]);
        tag(&mut sessions, 0, "GAN");
        let after = SolveSnapshot {
            time_ms: 99,
            scramble: "U".to_owned(),
            ..sessions[0].solves()[0].snapshot()
        };
        assert!(sessions[0].edit(0, after));
        assert_eq!(tags_of(&sessions), [vec!["GAN".to_owned()]]);
        saved(&path, &sessions);

        // Read back from JSON, the way times.json hands it over.
        let json = serde_json::to_string(&sessions).unwrap();
        let mut loaded: Vec<History> = serde_json::from_str(&json).unwrap();
        SolveExtras::open(&path, &mut loaded).unwrap();
        assert_eq!(tags_of(&loaded), [vec!["GAN".to_owned()]]);
        remove_folder_of(&path);
    }

    #[test]
    fn identical_solves_in_two_sessions_keep_their_own_tags() {
        let path = temporary_history();
        let mut sessions = [session(&[10]), session(&[10])].concat();
        tag(&mut sessions, 0, "GAN");
        saved(&path, &sessions);

        let mut loaded = [session(&[10]), session(&[10])].concat();
        SolveExtras::open(&path, &mut loaded).unwrap();
        assert_eq!(tags_of(&loaded[..1]), [vec!["GAN".to_owned()]]);
        assert_eq!(tags_of(&loaded[1..]), [Vec::<String>::new()]);
        remove_folder_of(&path);
    }

    #[test]
    fn a_solve_missing_from_the_history_keeps_its_entry_until_it_returns() {
        let path = temporary_history();
        let mut sessions = session(&[10, 20]);
        tag(&mut sessions, 1, "GAN");
        saved(&path, &sessions);

        let mut without = session(&[10]);
        let mut extras = SolveExtras::open(&path, &mut without).unwrap();
        extras.save(&without).unwrap();

        let mut back = session(&[10, 20]);
        extras.hand_over(&mut back);
        assert_eq!(tags_of(&back), [vec![], vec!["GAN".to_owned()]]);
        remove_folder_of(&path);
    }

    #[test]
    fn a_deleted_solve_loses_its_entry_and_undo_brings_it_back() {
        let path = temporary_history();
        let mut sessions = session(&[10, 20]);
        tag(&mut sessions, 1, "GAN");
        let mut extras = saved(&path, &sessions);
        extras.release_present(&sessions);

        let deleted = sessions[0].remove(1).unwrap();
        extras.save(&sessions).unwrap();
        let mut loaded = session(&[10, 20]);
        SolveExtras::open(&path, &mut loaded).unwrap();
        assert_eq!(
            tags_of(&loaded),
            [Vec::<String>::new(), Vec::<String>::new()]
        );

        sessions[0].insert(1, deleted);
        extras.save(&sessions).unwrap();
        let mut loaded = session(&[10, 20]);
        SolveExtras::open(&path, &mut loaded).unwrap();
        assert_eq!(tags_of(&loaded), [vec![], vec!["GAN".to_owned()]]);
        remove_folder_of(&path);
    }

    #[test]
    fn clearing_the_last_tag_is_saved() {
        let path = temporary_history();
        let mut sessions = session(&[10]);
        tag(&mut sessions, 0, "GAN");
        let mut extras = saved(&path, &sessions);
        assert!(sessions[0].set_tags(0, Vec::new()));
        extras.save(&sessions).unwrap();

        let mut loaded = session(&[10]);
        SolveExtras::open(&path, &mut loaded).unwrap();
        assert_eq!(tags_of(&loaded), [Vec::<String>::new()]);
        remove_folder_of(&path);
    }

    #[test]
    fn a_tag_change_reaches_the_entries_of_missing_solves() {
        let path = temporary_history();
        let mut sessions = session(&[10, 20]);
        tag(&mut sessions, 0, "GAN");
        tag(&mut sessions, 1, "OH");
        saved(&path, &sessions);

        let mut extras = SolveExtras::open(&path, &mut session(&[])).unwrap();
        extras.change_tags(&TagChange::Rename {
            from: "gan".to_owned(),
            to: "Moyu".to_owned(),
        });
        extras.change_tags(&TagChange::Remove {
            tag: "oh".to_owned(),
        });

        let mut back = session(&[10, 20]);
        extras.hand_over(&mut back);
        assert_eq!(
            tags_of(&back),
            [vec!["Moyu".to_owned()], Vec::<String>::new()]
        );
        remove_folder_of(&path);
    }

    #[test]
    fn an_unreadable_file_is_an_error_and_stays_on_disk() {
        let path = temporary_history();
        let file = path.with_file_name(FILE_NAME);
        for broken in [
            "not json",
            r#"{"solves":[{"tags":["no id"]}]}"#,
            r#"{"solves":[{"event":"Cube3x3","scramble":"R","solved_at_unix_ms":1,"time_ms":1,"occurrence":0,"tags":[" "]}]}"#,
        ] {
            fs::write(&file, broken).unwrap();
            let opened = SolveExtras::open(&path, &mut session(&[]));
            assert!(opened.is_err(), "{broken}");
            assert_eq!(fs::read_to_string(&file).unwrap(), broken);
        }
        remove_folder_of(&path);
    }
}
