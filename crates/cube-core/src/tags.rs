//! Tags on solves: what a valid tag is, how tags are matched and how they
//! filter a list of solves.
//!
//! Tags are matched without regard to case and keep the spelling they were
//! first typed with. They are stored in `solve-extras.json`, not in
//! `times.json`, because cube-tui drops the fields it does not know when it
//! saves. See [`solve_extras`](crate::solve_extras).

use crate::history::History;
use crate::solve::Solve;

/// The longest a tag may be, in characters.
pub const MAX_TAG_CHARS: usize = 32;

/// Why a tag cannot be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagError {
    /// The tag is blank, longer than [`MAX_TAG_CHARS`] or has control
    /// characters in it.
    Invalid,
    /// Another tag already has this name.
    Exists,
}

/// Reads a tag typed by the user, without the spaces around it.
pub fn parse(text: &str) -> Result<String, TagError> {
    let tag = text.trim();
    let valid = !tag.is_empty()
        && tag.chars().count() <= MAX_TAG_CHARS
        && !tag.chars().any(char::is_control);
    valid.then(|| tag.to_owned()).ok_or(TagError::Invalid)
}

/// Returns whether two tags are the same tag.
pub fn same(first: &str, second: &str) -> bool {
    first
        .chars()
        .flat_map(char::to_lowercase)
        .eq(second.chars().flat_map(char::to_lowercase))
}

/// Returns whether `tags` has `tag`.
pub fn has(tags: &[String], tag: &str) -> bool {
    tags.iter().any(|other| same(other, tag))
}

/// Returns `tag` as `known` spells it, or as it is when it is new.
pub fn spelled(known: &[String], tag: String) -> String {
    known
        .iter()
        .find(|other| same(other, &tag))
        .cloned()
        .unwrap_or(tag)
}

/// Adds `tag` to `tags`, or takes it out when it is there.
pub fn toggle(tags: &mut Vec<String>, tag: &str) {
    if has(tags, tag) {
        tags.retain(|other| !same(other, tag));
    } else {
        tags.push(tag.to_owned());
    }
}

/// A tag and how many solves have it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagUsage {
    pub tag: String,
    pub solves: usize,
}

/// Returns every tag in use or in `active`, sorted, each in the spelling met
/// first.
pub fn usage(sessions: &[History], active: &[String]) -> Vec<TagUsage> {
    let mut tags: Vec<TagUsage> = Vec::new();
    let mut count =
        |tag: &String, solves: usize| match tags.iter_mut().find(|usage| same(&usage.tag, tag)) {
            Some(usage) => usage.solves += solves,
            None => tags.push(TagUsage {
                tag: tag.clone(),
                solves,
            }),
        };
    let solves = sessions.iter().flat_map(History::solves);
    for tag in solves.flat_map(Solve::tags) {
        count(tag, 1);
    }
    for tag in active {
        count(tag, 0);
    }
    tags.sort_by_cached_key(|usage| usage.tag.to_lowercase());
    tags
}

/// Checks a new name for the tag `from`, returning it ready to use. Changing
/// only its case is allowed. Renaming onto another tag is not: the two could
/// never be told apart again.
pub fn check_rename(known: &[String], from: &str, text: &str) -> Result<String, TagError> {
    let to = parse(text)?;
    if !same(from, &to) && has(known, &to) {
        return Err(TagError::Exists);
    }
    Ok(to)
}

/// A change to a tag everywhere it is used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagChange {
    Rename { from: String, to: String },
    Remove { tag: String },
}

impl TagChange {
    /// Applies the change to the tags of one solve.
    pub fn apply(&self, tags: &mut Vec<String>) {
        match self {
            Self::Rename { from, to } => {
                for tag in tags.iter_mut().filter(|tag| same(tag, from)) {
                    tag.clone_from(to);
                }
            }
            Self::Remove { tag } => tags.retain(|other| !same(other, tag)),
        }
    }

    /// Returns what a filter set to `tag` should show after the change.
    #[must_use]
    pub fn follow(&self, tag: Option<String>) -> Option<String> {
        let tag = tag?;
        match self {
            Self::Rename { from, to } if same(&tag, from) => Some(to.clone()),
            Self::Remove { tag: removed } if same(&tag, removed) => None,
            _ => Some(tag),
        }
    }
}

/// Returns the filter after `current`: every solve, then each of `known` in
/// turn, then every solve again.
pub fn next_filter(current: Option<&str>, known: &[String]) -> Option<String> {
    match current {
        None => known.first().cloned(),
        Some(tag) => {
            let index = known.iter().position(|other| same(other, tag))?;
            known.get(index + 1).cloned()
        }
    }
}

/// Returns whether `solve` passes a filter, where `None` keeps every solve.
pub fn keeps(solve: &Solve, filter: Option<&str>) -> bool {
    filter.is_none_or(|tag| has(solve.tags(), tag))
}

/// Returns the indices of the solves a filter keeps, in order.
pub fn positions(solves: &[Solve], filter: Option<&str>) -> Vec<usize> {
    let kept = solves.iter().enumerate();
    kept.filter(|(_, solve)| keeps(solve, filter))
        .map(|(index, _)| index)
        .collect()
}

/// Returns the solves a filter keeps, in order.
pub fn filtered(solves: &[Solve], filter: Option<&str>) -> Vec<Solve> {
    let kept = solves.iter().filter(|solve| keeps(solve, filter));
    kept.cloned().collect()
}

/// Returns the last of the sorted `positions` no later than `index`, or the
/// first of them when all are later.
pub fn nearest(positions: &[usize], index: usize) -> Option<usize> {
    let after = positions.partition_point(|&position| position <= index);
    positions.get(after.saturating_sub(1)).copied()
}

/// Moves `selected`, a solve index, one step newer or older within the sorted
/// `positions`. With nothing selected it starts from the newest.
pub fn step(positions: &[usize], selected: Option<usize>, newer: bool) -> Option<usize> {
    let last = positions.len().checked_sub(1)?;
    let Some(selected) = selected.and_then(|index| nearest(positions, index)) else {
        return Some(positions[last]);
    };
    let place = positions.partition_point(|&position| position < selected);
    let place = if newer {
        (place + 1).min(last)
    } else {
        place.saturating_sub(1)
    };
    Some(positions[place])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::WcaEvent;
    use crate::solve::Modifier;

    fn tagged(ms: u64, tags: &[&str]) -> Solve {
        let mut solve = Solve::new(ms, WcaEvent::Cube3x3, format!("R{ms}"), Modifier::None);
        solve.set_tags(strings(tags));
        solve
    }

    fn strings(tags: &[&str]) -> Vec<String> {
        tags.iter().map(|&tag| tag.to_owned()).collect()
    }

    fn session(solves: Vec<Solve>) -> History {
        let mut history = History::new();
        for solve in solves {
            history.push(solve);
        }
        history
    }

    #[test]
    fn a_tag_is_trimmed_and_needs_visible_text_of_a_sane_length() {
        assert_eq!(parse("  GAN 12 ").unwrap(), "GAN 12");
        assert_eq!(parse("   "), Err(TagError::Invalid));
        assert_eq!(parse("a\nb"), Err(TagError::Invalid));
        assert!(parse(&"ñ".repeat(MAX_TAG_CHARS)).is_ok());
        assert_eq!(
            parse(&"a".repeat(MAX_TAG_CHARS + 1)),
            Err(TagError::Invalid)
        );
    }

    #[test]
    fn matching_ignores_case_in_any_script() {
        assert!(same("Warm-Up", "warm-UP"));
        assert!(same("ÑANDÚ", "ñandú"));
        assert!(!same("warm-up", "warmup"));
    }

    #[test]
    fn a_tag_keeps_the_spelling_it_was_first_typed_with() {
        let sessions = [session(vec![
            tagged(1, &["GAN"]),
            tagged(2, &["gan", "OH"]),
        ])];
        let usage = usage(&sessions, &strings(&["oh", "Blind"]));
        let known: Vec<String> = usage.into_iter().map(|usage| usage.tag).collect();
        assert_eq!(known, ["Blind", "GAN", "OH"]);
        assert_eq!(spelled(&known, "gan".to_owned()), "GAN");
        assert_eq!(spelled(&known, "new".to_owned()), "new");
    }

    #[test]
    fn toggling_adds_and_removes_whatever_the_case() {
        let mut tags = strings(&["GAN"]);
        toggle(&mut tags, "oh");
        assert_eq!(tags, ["GAN", "oh"]);
        toggle(&mut tags, "gan");
        assert_eq!(tags, ["oh"]);
    }

    #[test]
    fn usage_counts_solves_across_sessions_but_not_the_active_tags() {
        let sessions = [
            session(vec![tagged(1, &["GAN"]), tagged(2, &[])]),
            session(vec![tagged(3, &["gan", "OH"])]),
        ];
        let solves = |tag: &str| {
            let usage = usage(&sessions, &strings(&["Gan", "Blind"]));
            usage
                .iter()
                .find(|usage| same(&usage.tag, tag))
                .unwrap()
                .solves
        };
        assert_eq!(solves("Gan"), 2);
        assert_eq!(solves("OH"), 1);
        assert_eq!(solves("Blind"), 0);
    }

    #[test]
    fn renaming_onto_another_tag_is_refused_but_recasing_is_not() {
        let known = strings(&["GAN", "OH"]);
        assert_eq!(check_rename(&known, "GAN", "Moyu").unwrap(), "Moyu");
        assert_eq!(check_rename(&known, "GAN", "gan").unwrap(), "gan");
        assert_eq!(check_rename(&known, "GAN", "oh"), Err(TagError::Exists));
        assert_eq!(check_rename(&known, "GAN", " "), Err(TagError::Invalid));
    }

    #[test]
    fn a_rename_reaches_every_spelling_of_the_tag() {
        let change = TagChange::Rename {
            from: "gan".to_owned(),
            to: "Moyu".to_owned(),
        };
        let mut tags = strings(&["GAN", "OH"]);
        change.apply(&mut tags);
        assert_eq!(tags, ["Moyu", "OH"]);
        assert_eq!(change.follow(Some("Gan".to_owned())).unwrap(), "Moyu");
        assert_eq!(change.follow(Some("OH".to_owned())).unwrap(), "OH");
        assert_eq!(change.follow(None), None);
    }

    #[test]
    fn a_removal_clears_the_tag_and_any_filter_on_it() {
        let change = TagChange::Remove {
            tag: "gan".to_owned(),
        };
        let mut tags = strings(&["GAN", "OH"]);
        change.apply(&mut tags);
        assert_eq!(tags, ["OH"]);
        assert_eq!(change.follow(Some("GAN".to_owned())), None);
        assert_eq!(change.follow(Some("OH".to_owned())).unwrap(), "OH");
    }

    #[test]
    fn the_filter_cycles_through_every_tag_and_back() {
        let known = strings(&["GAN", "OH"]);
        assert_eq!(next_filter(None, &known).unwrap(), "GAN");
        assert_eq!(next_filter(Some("gan"), &known).unwrap(), "OH");
        assert_eq!(next_filter(Some("OH"), &known), None);
        assert_eq!(next_filter(Some("gone"), &known), None);
        assert_eq!(next_filter(None, &[]), None);
    }

    #[test]
    fn filtering_keeps_the_tagged_solves_in_order() {
        let solves = [
            tagged(1, &["GAN"]),
            tagged(2, &[]),
            tagged(3, &["OH", "gan"]),
        ];
        assert_eq!(positions(&solves, Some("Gan")), [0, 2]);
        assert_eq!(positions(&solves, None), [0, 1, 2]);
        let kept: Vec<u64> = filtered(&solves, Some("oh"))
            .iter()
            .map(Solve::raw_ms)
            .collect();
        assert_eq!(kept, [3]);
    }

    #[test]
    fn nearest_prefers_the_solve_at_or_before_the_index() {
        assert_eq!(nearest(&[2, 5, 9], 5), Some(5));
        assert_eq!(nearest(&[2, 5, 9], 7), Some(5));
        assert_eq!(nearest(&[2, 5, 9], 1), Some(2));
        assert_eq!(nearest(&[], 1), None);
    }

    #[test]
    fn stepping_stays_on_the_kept_solves_and_stops_at_both_ends() {
        let kept = [2, 5, 9];
        assert_eq!(step(&kept, None, false), Some(9));
        assert_eq!(step(&kept, Some(9), false), Some(5));
        assert_eq!(step(&kept, Some(7), false), Some(2));
        assert_eq!(step(&kept, Some(5), true), Some(9));
        assert_eq!(step(&kept, Some(9), true), Some(9));
        assert_eq!(step(&kept, Some(2), false), Some(2));
        assert_eq!(step(&[], Some(2), true), None);
    }
}
