//! Combines the history in memory with a `times.json` another app changed.
//!
//! cube-tui rewrites the whole file from what it has in memory, so a solve
//! missing from the file may be a deletion or may be a solve it never knew
//! about, and the two look the same. The merge therefore never lets the file
//! remove anything: a solve or session only goes away when this app deletes it.

use std::collections::{HashMap, HashSet};

use crate::event::WcaEvent;
use crate::history::History;
use crate::solve::Solve;

/// What a merge did, for telling the user.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MergeSummary {
    /// Solves taken from the file.
    pub added: usize,
    /// Solves replaced by a newer edit from the file.
    pub updated: usize,
    /// Solves saved earlier that the file no longer has, kept anyway.
    pub kept: usize,
}

impl MergeSummary {
    /// Returns whether no solve was affected.
    pub const fn is_empty(self) -> bool {
        self.added == 0 && self.updated == 0 && self.kept == 0
    }
}

/// Identifies a solve across both apps, which share no ids.
///
/// Built from the solve as first recorded, so edits do not change it.
/// `occurrence` tells identical legacy solves apart.
#[derive(Clone, PartialEq, Eq, Hash)]
struct SolveId {
    event: WcaEvent,
    scramble: String,
    solved_at_unix_ms: u64,
    time_ms: u64,
    occurrence: usize,
}

fn solve_ids(session: &History) -> Vec<SolveId> {
    let mut seen: HashMap<SolveId, usize> = HashMap::new();
    session
        .solves()
        .iter()
        .map(|solve| {
            let (event, scramble, solved_at_unix_ms, time_ms) = solve.origin();
            let mut id = SolveId {
                event,
                scramble: scramble.to_owned(),
                solved_at_unix_ms,
                time_ms,
                occurrence: 0,
            };
            let count = seen.entry(id.clone()).or_default();
            id.occurrence = *count;
            *count += 1;
            id
        })
        .collect()
}

/// Orders versions of one solve: the audit trail only ever grows, so an older
/// copy, such as one a stale cube-tui wrote back, always compares lower.
fn revision(solve: &Solve) -> (u64, usize) {
    let changes = solve.changes();
    let edited_at = changes.last().map_or(0, |change| change.changed_at_unix_ms);
    (edited_at, changes.len())
}

/// Pairs each session of `from` with one of `to`: first by the solves they
/// share, since names can change, then by name.
fn pair_sessions(from: &[History], to: &[History]) -> Vec<Option<usize>> {
    let mut owners: HashMap<SolveId, usize> = HashMap::new();
    for (index, session) in to.iter().enumerate() {
        owners.extend(solve_ids(session).into_iter().map(|id| (id, index)));
    }
    let mut pairs = vec![None; from.len()];
    let mut taken = vec![false; to.len()];
    for (pair, session) in pairs.iter_mut().zip(from) {
        let mut shared = vec![0_usize; to.len()];
        for id in solve_ids(session) {
            if let Some(&owner) = owners.get(&id) {
                shared[owner] += 1;
            }
        }
        let best = (0..to.len())
            .filter(|&index| !taken[index] && shared[index] > 0)
            .max_by_key(|&index| shared[index]);
        if let Some(best) = best {
            taken[best] = true;
            *pair = Some(best);
        }
    }
    for (pair, session) in pairs.iter_mut().zip(from) {
        if pair.is_none() {
            *pair =
                (0..to.len()).find(|&index| !taken[index] && to[index].name() == session.name());
            if let Some(index) = *pair {
                taken[index] = true;
            }
        }
    }
    pairs
}

/// Brings `theirs` into `ours`, with `base` as their last common version.
fn merge_session(
    base: Option<&History>,
    ours: &mut History,
    theirs: &History,
    summary: &mut MergeSummary,
) {
    let empty = History::new();
    let base = base.unwrap_or(&empty);
    if ours.name() == base.name() {
        ours.set_name(theirs.name().to_owned());
    }

    let known: HashMap<SolveId, &Solve> = solve_ids(base).into_iter().zip(base.solves()).collect();
    let their_ids = solve_ids(theirs);
    let their_solves: HashMap<&SolveId, &Solve> = their_ids.iter().zip(theirs.solves()).collect();

    let our_ids = solve_ids(ours);
    for (id, solve) in our_ids.iter().zip(ours.solves_mut()) {
        match their_solves.get(id) {
            Some(their) if revision(their) > revision(solve) => {
                solve.clone_from(their);
                summary.updated += 1;
            }
            None if known.contains_key(id) => summary.kept += 1,
            _ => {}
        }
    }

    let our_ids: HashSet<SolveId> = our_ids.into_iter().collect();
    for (id, solve) in their_ids.iter().zip(theirs.solves()) {
        if our_ids.contains(id) {
            continue;
        }
        // In the base but not here means it was deleted here. It stays
        // deleted unless the other app has edited it since.
        if known
            .get(id)
            .is_none_or(|known| revision(solve) > revision(known))
        {
            ours.insert_chronologically(solve.clone());
            summary.added += 1;
        }
    }
    ours.remark_single_records();
}

/// Merges `theirs` into `ours`, with `base` as the version both started from.
///
/// Sessions in `ours` keep their positions and new ones are appended, so
/// session indices into `ours` stay valid. Solve indices may not.
pub fn merge(base: &[History], ours: &mut Vec<History>, theirs: &[History]) -> MergeSummary {
    let mut summary = MergeSummary::default();
    let our_pairs = pair_sessions(ours, theirs);
    let our_bases = pair_sessions(ours, base);
    let their_bases = pair_sessions(theirs, base);

    let mut claimed = vec![false; theirs.len()];
    for (index, session) in ours.iter_mut().enumerate() {
        if let Some(their) = our_pairs[index] {
            claimed[their] = true;
            let known = our_bases[index].or(their_bases[their]).map(|i| &base[i]);
            merge_session(known, session, &theirs[their], &mut summary);
        } else {
            let mut missing = History::new();
            missing.set_name(session.name().to_owned());
            let known = our_bases[index].map(|i| &base[i]);
            merge_session(known, session, &missing, &mut summary);
        }
    }

    for (index, session) in theirs.iter().enumerate() {
        if claimed[index] {
            continue;
        }
        let known = their_bases[index].map(|i| &base[i]);
        let mut incoming = History::new();
        incoming.set_name(session.name().to_owned());
        merge_session(known, &mut incoming, session, &mut summary);
        // A session deleted here returns only for what was added to it since.
        if known.is_none() || !incoming.is_empty() {
            ours.push(incoming);
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solve::{Modifier, SolveSnapshot};

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

    fn session(name: &str, times: &[u64]) -> History {
        let mut history = History::new();
        history.set_name(name.to_owned());
        for &ms in times {
            history.push(solve(ms));
        }
        history
    }

    fn times(session: &History) -> Vec<u64> {
        session.solves().iter().map(Solve::raw_ms).collect()
    }

    fn with_comment(history: &mut History, index: usize, comment: &str) {
        let after = SolveSnapshot {
            comment: comment.to_owned(),
            ..history.solves()[index].snapshot()
        };
        assert!(history.edit(index, after));
    }

    #[test]
    fn solves_from_both_apps_end_up_in_order() {
        let base = [session("", &[10, 40])];
        let mut ours = vec![session("", &[10, 40, 50])];
        let theirs = [session("", &[10, 20, 40])];
        let summary = merge(&base, &mut ours, &theirs);
        assert_eq!(times(&ours[0]), [10, 20, 40, 50]);
        assert_eq!(summary.added, 1);
        assert_eq!(summary.kept, 0);
    }

    #[test]
    fn a_stale_overwrite_cannot_drop_saved_solves() {
        // cube-tui started before solve 30 and wrote its own 40 over it.
        let base = [session("", &[10, 30])];
        let mut ours = vec![session("", &[10, 30])];
        let theirs = [session("", &[10, 40])];
        let summary = merge(&base, &mut ours, &theirs);
        assert_eq!(times(&ours[0]), [10, 30, 40]);
        assert_eq!(
            summary,
            MergeSummary {
                added: 1,
                updated: 0,
                kept: 1
            }
        );
    }

    #[test]
    fn a_stale_overwrite_cannot_undo_an_edit() {
        let mut edited = session("", &[10]);
        with_comment(&mut edited, 0, "lucky");
        let base = [edited.clone()];
        let mut ours = vec![edited];
        let summary = merge(&base, &mut ours, &[session("", &[10])]);
        assert_eq!(ours[0].solves()[0].comment(), "lucky");
        assert!(summary.is_empty());
    }

    #[test]
    fn edits_from_the_other_app_are_taken() {
        let original = session("", &[10, 20]);
        let mut theirs = original.clone();
        with_comment(&mut theirs, 0, "from tui");
        let mut ours = vec![original.clone()];
        let summary = merge(&[original], &mut ours, &[theirs]);
        assert_eq!(ours[0].solves()[0].comment(), "from tui");
        assert_eq!(ours[0].solves()[0].changes().len(), 1);
        assert_eq!(summary.updated, 1);
    }

    #[test]
    fn a_solve_deleted_here_stays_deleted() {
        let base = [session("", &[10, 20])];
        let mut ours = vec![session("", &[10])];
        let theirs = [session("", &[10, 20, 30])];
        merge(&base, &mut ours, &theirs);
        assert_eq!(times(&ours[0]), [10, 30]);
    }

    #[test]
    fn an_edit_survives_a_deletion_here() {
        let original = session("", &[10, 20]);
        let mut theirs = original.clone();
        with_comment(&mut theirs, 1, "keep me");
        let mut ours = vec![session("", &[10])];
        merge(&[original], &mut ours, &[theirs]);
        assert_eq!(times(&ours[0]), [10, 20]);
        assert_eq!(ours[0].solves()[1].comment(), "keep me");
    }

    #[test]
    fn identical_legacy_solves_are_counted_separately() {
        let base = [session("", &[10, 10])];
        let mut ours = vec![session("", &[10, 10])];
        let theirs = [session("", &[10, 10, 10])];
        merge(&base, &mut ours, &theirs);
        assert_eq!(times(&ours[0]), [10, 10, 10]);
    }

    #[test]
    fn sessions_pair_by_solves_when_renamed_or_moved() {
        let base = [session("a", &[10]), session("b", &[20])];
        let mut ours = vec![session("a", &[10]), session("b", &[20])];
        let theirs = [session("renamed", &[20, 25]), session("a", &[10])];
        merge(&base, &mut ours, &theirs);
        assert_eq!(ours.len(), 2);
        assert_eq!(ours[1].name(), "renamed");
        assert_eq!(times(&ours[1]), [20, 25]);
        assert_eq!(times(&ours[0]), [10]);
    }

    #[test]
    fn a_rename_here_wins_over_the_old_name() {
        let base = [session("old", &[10])];
        let mut ours = vec![session("new", &[10])];
        merge(&base, &mut ours, &[session("old", &[10])]);
        assert_eq!(ours[0].name(), "new");
    }

    #[test]
    fn new_sessions_are_appended_and_existing_ones_keep_their_place() {
        let base = [session("a", &[10])];
        let mut ours = vec![session("a", &[10]), session("mine", &[20])];
        let theirs = [session("a", &[10]), session("theirs", &[30])];
        merge(&base, &mut ours, &theirs);
        let names: Vec<&str> = ours.iter().map(History::name).collect();
        assert_eq!(names, ["a", "mine", "theirs"]);
    }

    #[test]
    fn a_session_missing_from_the_file_is_kept() {
        let base = [session("a", &[10]), session("b", &[20])];
        let mut ours = base.to_vec();
        let summary = merge(&base, &mut ours, &[session("a", &[10])]);
        assert_eq!(ours.len(), 2);
        assert_eq!(ours[1].name(), "b");
        assert_eq!(summary.kept, 1);
    }

    #[test]
    fn a_session_deleted_here_returns_only_for_new_solves() {
        let base = [session("a", &[10]), session("b", &[20])];
        let theirs = [session("a", &[10]), session("b", &[20])];
        let mut ours = vec![session("a", &[10])];
        merge(&base, &mut ours, &theirs);
        assert_eq!(ours.len(), 1);

        let theirs = [session("a", &[10]), session("b", &[20, 30])];
        merge(&base, &mut ours, &theirs);
        assert_eq!(ours.len(), 2);
        assert_eq!(times(&ours[1]), [30]);
    }

    #[test]
    fn records_are_recomputed_after_merging() {
        let base = [session("", &[50])];
        let mut ours = vec![session("", &[50, 60])];
        let theirs = [session("", &[40, 50])];
        merge(&base, &mut ours, &theirs);
        let records: Vec<bool> = ours[0]
            .solves()
            .iter()
            .map(Solve::was_single_record)
            .collect();
        assert_eq!(times(&ours[0]), [40, 50, 60]);
        assert_eq!(records, [true, false, false]);
    }
}
