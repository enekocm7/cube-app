//! Relays: several puzzles timed as one attempt, with a split for each.
//!
//! A relay has several events and a scramble for each, so it does not fit a
//! [`Solve`](crate::solve::Solve). Relays live in `relays.json`, never in
//! `times.json`, which is cube-tui's file, and they never reach the per-event
//! statistics.
//!
//! The rules:
//! - There is one inspection for the whole relay, taken before the first
//!   puzzle. A `+2` from it counts against the first puzzle, and a DNF from it
//!   makes the whole relay a DNF.
//! - A key press ends each puzzle but the last (see [`crate::timer::Splits`]).
//! - Any puzzle can carry a `+2`, which adds two seconds to its split and so to
//!   the total, or a DNF, which makes the total a DNF. The splits, penalties
//!   included, always add up to the total.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::event::WcaEvent;
use crate::format::parse_time;
use crate::scramble;
use crate::solve::Modifier;
use crate::timer::MAX_PUZZLES;

/// The fewest puzzles a relay may have.
pub const MIN_EVENTS: usize = 2;

/// The most puzzles a relay may have.
pub const MAX_EVENTS: usize = MAX_PUZZLES;

/// What a `+2` adds, in milliseconds.
const PLUS_TWO_MS: u64 = 2_000;

/// Why a list of events is not a relay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelayError {
    /// Fewer than [`MIN_EVENTS`] events.
    TooFew,
    /// More than [`MAX_EVENTS`] events.
    TooMany,
    /// This text names no event.
    Unknown(String),
    /// Blindfolded events are timed in two parts and cannot be relayed.
    Blindfolded(WcaEvent),
}

/// Checks that `events` make a relay.
///
/// # Errors
/// Says what is wrong with the list.
pub fn validate(events: &[WcaEvent]) -> Result<(), RelayError> {
    if events.len() < MIN_EVENTS {
        return Err(RelayError::TooFew);
    }
    if events.len() > MAX_EVENTS {
        return Err(RelayError::TooMany);
    }
    match events.iter().find(|event| event.is_blindfolded()) {
        Some(&event) => Err(RelayError::Blindfolded(event)),
        None => Ok(()),
    }
}

/// Reads the events of a relay as typed: names separated by commas, in any
/// case, such as `2x2x2, 3x3x3, Pyraminx`. A range of cubes such as
/// `2x2x2-5x5x5` stands for every cube from one to the other.
///
/// # Errors
/// Says what is wrong with the text.
pub fn parse_events(text: &str) -> Result<Vec<WcaEvent>, RelayError> {
    let mut events = Vec::new();
    for part in text.split([',', ';', '\n']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(event) = find_event(part) {
            events.push(event);
        } else if let Some(range) = parse_range(part) {
            events.extend(range);
        } else {
            return Err(RelayError::Unknown(part.to_owned()));
        }
    }
    validate(&events)?;
    Ok(events)
}

fn find_event(text: &str) -> Option<WcaEvent> {
    let key = |text: &str| {
        text.chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    let wanted = key(text);
    WcaEvent::ALL
        .into_iter()
        .find(|event| key(event.name()) == wanted)
}

fn parse_range(text: &str) -> Option<Vec<WcaEvent>> {
    let (from, to) = text.split_once('-').or_else(|| text.split_once(" to "))?;
    let (from, to) = (find_event(from.trim())?, find_event(to.trim())?);
    let (from, to) = (cube_size(from)?, cube_size(to)?);
    if from > to {
        return None;
    }
    Some(CUBES[usize::from(from - 2)..=usize::from(to - 2)].to_vec())
}

const CUBES: [WcaEvent; 6] = [
    WcaEvent::Cube2x2,
    WcaEvent::Cube3x3,
    WcaEvent::Cube4x4,
    WcaEvent::Cube5x5,
    WcaEvent::Cube6x6,
    WcaEvent::Cube7x7,
];

const fn cube_size(event: WcaEvent) -> Option<u8> {
    match event {
        WcaEvent::Cube2x2 => Some(2),
        WcaEvent::Cube3x3 => Some(3),
        WcaEvent::Cube4x4 => Some(4),
        WcaEvent::Cube5x5 => Some(5),
        WcaEvent::Cube6x6 => Some(6),
        WcaEvent::Cube7x7 => Some(7),
        _ => None,
    }
}

/// Returns the name of a relay: `2x2x2 to 5x5x5` for a run of consecutive
/// cubes, and the events joined with `+` otherwise.
pub fn name(events: &[WcaEvent]) -> String {
    let sizes: Option<Vec<u8>> = events.iter().map(|&event| cube_size(event)).collect();
    if let Some(sizes) = sizes
        && sizes.len() >= 3
        && sizes.windows(2).all(|pair| pair[1] == pair[0] + 1)
    {
        return format!(
            "{} to {}",
            events[0].name(),
            events[events.len() - 1].name()
        );
    }
    let names: Vec<&str> = events.iter().map(|event| event.name()).collect();
    names.join(" + ")
}

/// The relays offered without being created: runs of cubes, the cubes with a
/// pair of puzzles, and the other puzzles together.
pub fn presets() -> Vec<Vec<WcaEvent>> {
    let cubes = |first: usize, last: usize| CUBES[first - 2..=last - 2].to_vec();
    vec![
        cubes(2, 4),
        cubes(2, 5),
        cubes(2, 6),
        cubes(2, 7),
        cubes(3, 5),
        cubes(3, 7),
        cubes(4, 7),
        cubes(2, 3),
        vec![WcaEvent::Cube3x3, WcaEvent::Cube3x3OneHanded],
        vec![
            WcaEvent::Cube2x2,
            WcaEvent::Cube3x3,
            WcaEvent::Pyraminx,
            WcaEvent::Skewb,
        ],
        vec![WcaEvent::Megaminx, WcaEvent::Pyraminx, WcaEvent::Skewb],
        vec![
            WcaEvent::Cube3x3,
            WcaEvent::Square1,
            WcaEvent::Clock,
            WcaEvent::Megaminx,
        ],
    ]
}

/// One relay attempt, recorded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayAttempt {
    /// The events of the relay, in the order solved.
    pub events: Vec<WcaEvent>,
    /// The time on each puzzle without penalties, in milliseconds.
    pub splits_ms: Vec<u64>,
    /// The penalty of each puzzle.
    pub modifiers: Vec<Modifier>,
    #[serde(default)]
    pub solved_at_unix_ms: u64,
    /// The scramble of each puzzle.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scrambles: Vec<String>,
    /// The total of an attempt a hardware timer timed, which reports no
    /// per-puzzle splits, or `None` for an attempt timed with splits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_only_ms: Option<u64>,
}

impl RelayAttempt {
    /// Records an attempt from the splits the timer took.
    ///
    /// `inspection` is the penalty the one inspection earned. A DNF makes the
    /// whole relay a DNF, which is held by the first puzzle.
    pub fn new(
        events: Vec<WcaEvent>,
        scrambles: Vec<String>,
        splits_ms: Vec<u64>,
        inspection: Modifier,
        solved_at_unix_ms: u64,
    ) -> Self {
        let mut modifiers = vec![Modifier::None; events.len()];
        if let Some(first) = modifiers.first_mut() {
            *first = inspection;
        }
        Self {
            events,
            splits_ms,
            modifiers,
            solved_at_unix_ms,
            scrambles,
            total_only_ms: None,
        }
    }

    /// Records an attempt a hardware timer timed, which has a total but no
    /// splits: the timer reports only the total, and the key that passes a
    /// puzzle marks no time.
    pub fn timed_by_hardware(
        events: Vec<WcaEvent>,
        scrambles: Vec<String>,
        total_ms: u64,
        solved_at_unix_ms: u64,
    ) -> Self {
        let mut attempt = Self::new(
            events,
            scrambles,
            Vec::new(),
            Modifier::None,
            solved_at_unix_ms,
        );
        attempt.total_only_ms = Some(total_ms);
        attempt
    }

    /// Returns whether the attempt has a time for each puzzle, rather than
    /// just its total.
    pub const fn has_splits(&self) -> bool {
        self.total_only_ms.is_none()
    }

    /// Returns the number of puzzles.
    pub fn puzzles(&self) -> usize {
        self.events.len()
    }

    /// Returns the time of puzzle `index` with its penalty, or `None` for a DNF
    /// or an attempt with only a total.
    pub fn split_ms(&self, index: usize) -> Option<u64> {
        if self.total_only_ms.is_some() {
            return None;
        }
        let raw = *self.splits_ms.get(index)?;
        penalized_ms(raw, self.modifiers.get(index).copied().unwrap_or_default())
    }

    /// Returns the total, which is the sum of the splits, or the time the
    /// hardware reported when the attempt has no splits, its penalties
    /// included. `None` when any puzzle is a DNF.
    pub fn total_ms(&self) -> Option<u64> {
        if let Some(total) = self.total_only_ms {
            let mut total = total;
            for &modifier in &self.modifiers {
                match modifier {
                    Modifier::None => {}
                    Modifier::PlusTwo => total += PLUS_TWO_MS,
                    Modifier::DNF => return None,
                }
            }
            return Some(total);
        }
        (0..self.puzzles()).try_fold(0, |total, index| Some(total + self.split_ms(index)?))
    }

    /// Returns whether the relay counts as a DNF.
    pub fn is_dnf(&self) -> bool {
        self.total_ms().is_none()
    }

    /// Gives puzzle `index` the penalty `modifier`, or takes it away when it
    /// already has it.
    pub fn toggle_penalty(&mut self, index: usize, modifier: Modifier) {
        // An attempt with only a total has no puzzle times to penalize.
        if self.total_only_ms.is_some() {
            return;
        }
        if let Some(current) = self.modifiers.get_mut(index) {
            *current = if *current == modifier {
                Modifier::None
            } else {
                modifier
            };
        }
    }

    /// Replaces the splits and penalties with what the edit dialog holds.
    ///
    /// # Errors
    /// The index of the puzzle whose split is not a time, in which case
    /// nothing changes. The lists must hold one entry per puzzle.
    pub fn edit(&mut self, splits: &[String], modifiers: &[Modifier]) -> Result<(), usize> {
        if splits.len() != self.puzzles() || modifiers.len() != self.puzzles() {
            return Err(0);
        }
        // An attempt with only a total may keep it: with every split left
        // empty the penalties still change, and the total stays.
        if self.total_only_ms.is_some() && splits.iter().all(|text| text.trim().is_empty()) {
            self.modifiers.clone_from_slice(modifiers);
            return Ok(());
        }
        let splits_ms = parse_splits(splits)?;
        self.splits_ms = splits_ms;
        self.modifiers.clone_from_slice(modifiers);
        // The attempt now has a split for every puzzle, so it is no longer an
        // attempt that only has a total.
        self.total_only_ms = None;
        Ok(())
    }

    /// Checks that the lists agree, so a file the app could not have written
    /// is refused rather than rewritten.
    pub fn is_consistent(&self) -> bool {
        let splits_agree = match self.total_only_ms {
            Some(_) => self.splits_ms.is_empty(),
            None => self.splits_ms.len() == self.events.len(),
        };
        validate(&self.events).is_ok()
            && splits_agree
            && self.modifiers.len() == self.events.len()
            && (self.scrambles.is_empty() || self.scrambles.len() == self.events.len())
    }
}

/// Returns `raw` with `modifier` applied, or `None` when the puzzle is a DNF.
fn penalized_ms(raw: u64, modifier: Modifier) -> Option<u64> {
    match modifier {
        Modifier::None => Some(raw),
        Modifier::PlusTwo => Some(raw + PLUS_TWO_MS),
        Modifier::DNF => None,
    }
}

/// Returns the total of `splits_ms` with their `modifiers`, or `None` when a
/// puzzle is a DNF or the lists differ in length.
pub fn total_of(splits_ms: &[u64], modifiers: &[Modifier]) -> Option<u64> {
    if splits_ms.len() != modifiers.len() {
        return None;
    }
    splits_ms
        .iter()
        .zip(modifiers)
        .try_fold(0, |total, (&raw, &modifier)| {
            Some(total + penalized_ms(raw, modifier)?)
        })
}

/// Parses the split typed for each puzzle in the edit dialog.
///
/// # Errors
/// The index of the first puzzle whose split is not a time above zero.
pub fn parse_splits(texts: &[String]) -> Result<Vec<u64>, usize> {
    texts
        .iter()
        .enumerate()
        .map(|(index, text)| parse_time(text).filter(|&ms| ms > 0).ok_or(index))
        .collect()
}

/// The contents of `relays.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RelayData {
    /// The relays created by hand; the presets are not stored.
    pub custom: Vec<Vec<WcaEvent>>,
    /// Every attempt, oldest first.
    pub attempts: Vec<RelayAttempt>,
}

impl RelayData {
    /// Checks the relays and attempts, returning the position of the first
    /// attempt that is not consistent (or the attempt count for a bad relay).
    pub fn validate(&self) -> Result<(), usize> {
        let bad_relay = self.custom.iter().any(|events| validate(events).is_err());
        match self.attempts.iter().position(|a| !a.is_consistent()) {
            Some(index) => Err(index),
            None if bad_relay => Err(self.attempts.len()),
            None => Ok(()),
        }
    }

    /// Returns every relay on offer: the presets, then the custom ones.
    pub fn relays(&self) -> Vec<Vec<WcaEvent>> {
        let mut relays = presets();
        for events in &self.custom {
            if !relays.contains(events) {
                relays.push(events.clone());
            }
        }
        relays
    }

    /// Summarizes the attempts at the relay with these events.
    pub fn summary(&self, events: &[WcaEvent]) -> RelaySummary {
        let mut summary = RelaySummary::default();
        let mut sum = 0_u64;
        for attempt in self.attempts.iter().filter(|a| a.events == events) {
            summary.attempts += 1;
            if let Some(total) = attempt.total_ms() {
                summary.finished += 1;
                sum += total;
                summary.best = Some(summary.best.map_or(total, |best| best.min(total)));
            }
        }
        if summary.finished > 0 {
            summary.mean = Some(sum / summary.finished as u64);
        }
        summary
    }
}

/// The figures of one relay.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RelaySummary {
    pub attempts: usize,
    /// The attempts that were not DNF.
    pub finished: usize,
    pub best: Option<u64>,
    /// The mean of the attempts that were not DNF.
    pub mean: Option<u64>,
}

/// Prepares a scramble for each event, which takes a while.
pub fn generate_scrambles(events: &[WcaEvent]) -> Vec<String> {
    events
        .iter()
        .map(|&event| scramble::generate(event))
        .collect()
}

/// Writes the scrambles as a numbered list with the event of each.
pub fn scrambles_text(events: &[WcaEvent], scrambles: &[String]) -> String {
    let mut text = String::new();
    for (index, (event, scramble)) in events.iter().zip(scrambles).enumerate() {
        let _ = writeln!(text, "{}. {}: {}", index + 1, event.name(), scramble.trim());
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attempt(splits: [u64; 3], modifiers: [Modifier; 3]) -> RelayAttempt {
        let mut attempt = RelayAttempt::new(
            presets()[0].clone(),
            Vec::new(),
            splits.to_vec(),
            Modifier::None,
            0,
        );
        attempt.modifiers = modifiers.to_vec();
        attempt
    }

    #[test]
    fn presets_run_from_2x2x2_to_4x4x4_up_to_7x7x7() {
        let presets = presets();
        assert_eq!(name(&presets[4]), "3x3x3 to 5x5x5");
        assert_eq!(name(&presets[6]), "4x4x4 to 7x7x7");
        let mut distinct = presets.clone();
        distinct.dedup();
        assert_eq!(distinct.len(), presets.len());
        assert_eq!(name(&presets[0]), "2x2x2 to 4x4x4");
        assert_eq!(name(&presets[3]), "2x2x2 to 7x7x7");
        assert_eq!(presets[3].len(), 6);
        assert!(presets.iter().all(|events| validate(events).is_ok()));
    }

    #[test]
    fn events_are_read_from_names_and_ranges() {
        assert_eq!(
            parse_events("2x2x2, 3x3x3, pyraminx").unwrap(),
            vec![WcaEvent::Cube2x2, WcaEvent::Cube3x3, WcaEvent::Pyraminx]
        );
        assert_eq!(parse_events("2x2x2-4x4x4").unwrap(), presets()[0]);
        assert_eq!(
            parse_events("square-1, 3x3x3 oh").unwrap(),
            vec![WcaEvent::Square1, WcaEvent::Cube3x3OneHanded]
        );
    }

    #[test]
    fn bad_lists_say_what_is_wrong() {
        assert_eq!(parse_events("3x3x3"), Err(RelayError::TooFew));
        assert_eq!(parse_events(""), Err(RelayError::TooFew));
        assert_eq!(
            parse_events("3x3x3, nope"),
            Err(RelayError::Unknown("nope".to_owned()))
        );
        assert_eq!(
            parse_events("3x3x3, 3x3x3 bld"),
            Err(RelayError::Blindfolded(WcaEvent::Cube3x3Blind))
        );
        let many = vec!["3x3x3"; MAX_EVENTS + 1].join(",");
        assert_eq!(parse_events(&many), Err(RelayError::TooMany));
    }

    #[test]
    fn other_relays_are_named_by_their_events() {
        assert_eq!(name(&[WcaEvent::Cube3x3, WcaEvent::Skewb]), "3x3x3 + Skewb");
        assert_eq!(
            name(&[WcaEvent::Cube2x2, WcaEvent::Cube3x3]),
            "2x2x2 + 3x3x3"
        );
    }

    #[test]
    fn splits_add_up_to_the_total() {
        let attempt = attempt([1_000, 2_000, 3_000], [Modifier::None; 3]);
        assert_eq!(attempt.total_ms(), Some(6_000));
    }

    #[test]
    fn a_plus_two_adds_to_its_split_and_to_the_total() {
        let attempt = attempt(
            [1_000, 2_000, 3_000],
            [Modifier::None, Modifier::PlusTwo, Modifier::None],
        );
        assert_eq!(attempt.split_ms(1), Some(4_000));
        let sum: u64 = (0..3).map(|i| attempt.split_ms(i).unwrap()).sum();
        assert_eq!(attempt.total_ms(), Some(sum));
        assert_eq!(sum, 8_000);
    }

    #[test]
    fn one_dnf_puzzle_makes_the_relay_a_dnf() {
        let attempt = attempt(
            [1_000, 2_000, 3_000],
            [Modifier::None, Modifier::None, Modifier::DNF],
        );
        assert!(attempt.is_dnf());
        assert_eq!(attempt.split_ms(0), Some(1_000));
    }

    #[test]
    fn the_inspection_penalty_lands_on_the_first_puzzle() {
        let attempt = RelayAttempt::new(
            presets()[0].clone(),
            Vec::new(),
            vec![1_000, 2_000, 3_000],
            Modifier::PlusTwo,
            0,
        );
        assert_eq!(attempt.modifiers[0], Modifier::PlusTwo);
        assert_eq!(attempt.total_ms(), Some(8_000));
    }

    #[test]
    fn toggling_a_penalty_sets_and_clears_it() {
        let mut attempt = attempt([1_000, 2_000, 3_000], [Modifier::None; 3]);
        attempt.toggle_penalty(2, Modifier::PlusTwo);
        assert_eq!(attempt.modifiers[2], Modifier::PlusTwo);
        attempt.toggle_penalty(2, Modifier::DNF);
        assert_eq!(attempt.modifiers[2], Modifier::DNF);
        attempt.toggle_penalty(2, Modifier::DNF);
        assert_eq!(attempt.modifiers[2], Modifier::None);
        attempt.toggle_penalty(9, Modifier::DNF);
    }

    #[test]
    fn an_edit_replaces_the_splits_and_penalties() {
        let mut attempt = attempt([1_000, 2_000, 3_000], [Modifier::None; 3]);
        let modifiers = [Modifier::PlusTwo, Modifier::None, Modifier::DNF];
        let texts = ["1.5".to_owned(), "2:03.250".to_owned(), "0:04".to_owned()];
        assert_eq!(attempt.edit(&texts, &modifiers), Ok(()));
        assert_eq!(attempt.splits_ms, vec![1_500, 123_250, 4_000]);
        assert_eq!(attempt.modifiers, modifiers);
        assert_eq!(attempt.total_ms(), None);
    }

    #[test]
    fn an_edit_with_a_bad_split_says_which_puzzle_and_changes_nothing() {
        let mut attempt = attempt([1_000, 2_000, 3_000], [Modifier::None; 3]);
        let texts = ["1.5".to_owned(), "nope".to_owned(), "0:04".to_owned()];
        assert_eq!(attempt.edit(&texts, &[Modifier::PlusTwo; 3]), Err(1));
        assert_eq!(attempt.splits_ms, vec![1_000, 2_000, 3_000]);
        assert_eq!(attempt.modifiers, [Modifier::None; 3]);
        let texts = ["1.5".to_owned(), "0.000".to_owned(), "0:04".to_owned()];
        assert_eq!(attempt.edit(&texts, &[Modifier::None; 3]), Err(1));
    }

    #[test]
    fn an_edit_needs_one_split_and_penalty_per_puzzle() {
        let mut attempt = attempt([1_000, 2_000, 3_000], [Modifier::None; 3]);
        assert_eq!(
            attempt.edit(&["1.5".to_owned()], &[Modifier::None; 3]),
            Err(0)
        );
        let texts = ["1.5".to_owned(), "2.0".to_owned(), "3.0".to_owned()];
        assert_eq!(attempt.edit(&texts, &[Modifier::None; 2]), Err(0));
    }

    #[test]
    fn the_total_of_edited_splits_follows_their_penalties() {
        let splits = [1_000, 2_000, 3_000];
        assert_eq!(total_of(&splits, &[Modifier::None; 3]), Some(6_000));
        assert_eq!(
            total_of(
                &splits,
                &[Modifier::PlusTwo, Modifier::None, Modifier::None]
            ),
            Some(8_000)
        );
        assert_eq!(
            total_of(&splits, &[Modifier::None, Modifier::DNF, Modifier::None]),
            None
        );
        assert_eq!(total_of(&splits, &[Modifier::None; 2]), None);
    }

    #[test]
    fn a_hardware_attempt_keeps_its_total_and_no_splits() {
        let mut attempt = RelayAttempt::timed_by_hardware(
            presets()[0].clone(),
            vec!["a".into(), "b".into(), "c".into()],
            22_118,
            0,
        );
        assert!(!attempt.has_splits());
        assert_eq!(attempt.split_ms(0), None);
        assert_eq!(attempt.total_ms(), Some(22_118));
        assert!(!attempt.is_dnf());
        assert!(attempt.is_consistent());
        // There are no puzzle times to penalize.
        attempt.toggle_penalty(0, Modifier::DNF);
        assert_eq!(attempt.modifiers[0], Modifier::None);
        assert_eq!(attempt.total_ms(), Some(22_118));
    }

    #[test]
    fn editing_a_hardware_attempt_gives_it_splits() {
        let mut attempt =
            RelayAttempt::timed_by_hardware(presets()[0].clone(), Vec::new(), 9_000, 0);
        let texts = ["1.0".to_owned(), "2.0".to_owned(), "3.0".to_owned()];
        assert_eq!(attempt.edit(&texts, &[Modifier::None; 3]), Ok(()));
        assert!(attempt.has_splits());
        assert_eq!(attempt.total_ms(), Some(6_000));
        assert!(attempt.is_consistent());
    }

    #[test]
    fn a_hardware_attempt_keeps_its_total_when_edited_without_splits() {
        let empty = [String::new(), String::new(), String::new()];
        let mut attempt =
            RelayAttempt::timed_by_hardware(presets()[0].clone(), Vec::new(), 22_118, 0);
        let mut modifiers = [Modifier::None; 3];
        modifiers[1] = Modifier::PlusTwo;
        assert_eq!(attempt.edit(&empty, &modifiers), Ok(()));
        assert!(!attempt.has_splits());
        assert_eq!(attempt.total_ms(), Some(24_118));
        assert!(attempt.is_consistent());
        modifiers[1] = Modifier::DNF;
        assert_eq!(attempt.edit(&empty, &modifiers), Ok(()));
        assert_eq!(attempt.total_ms(), None);
    }

    #[test]
    fn best_and_mean_are_per_relay_and_skip_dnfs() {
        let relay = presets()[0].clone();
        let other = presets()[1].clone();
        let mut data = RelayData::default();
        for (splits, modifier) in [
            ([1_000, 2_000, 3_000], Modifier::None),
            ([2_000, 3_000, 5_000], Modifier::None),
            ([1_000, 1_000, 1_000], Modifier::DNF),
        ] {
            data.attempts.push(RelayAttempt::new(
                relay.clone(),
                Vec::new(),
                splits.to_vec(),
                modifier,
                0,
            ));
        }
        let summary = data.summary(&relay);
        assert_eq!(summary.attempts, 3);
        assert_eq!(summary.finished, 2);
        assert_eq!(summary.best, Some(6_000));
        assert_eq!(summary.mean, Some(8_000));
        assert_eq!(data.summary(&other), RelaySummary::default());
    }

    #[test]
    fn custom_relays_follow_the_presets_without_repeating_them() {
        let data = RelayData {
            custom: vec![presets()[0].clone(), vec![WcaEvent::Skewb, WcaEvent::Clock]],
            attempts: Vec::new(),
        };
        assert_eq!(data.relays().len(), presets().len() + 1);
    }

    #[test]
    fn inconsistent_attempts_are_refused() {
        let mut data = RelayData::default();
        data.attempts.push(attempt([1, 2, 3], [Modifier::None; 3]));
        assert!(data.validate().is_ok());
        data.attempts[0].splits_ms.pop();
        assert_eq!(data.validate(), Err(0));
    }

    #[test]
    fn scrambles_are_listed_with_their_events() {
        let text = scrambles_text(
            &[WcaEvent::Cube2x2, WcaEvent::Cube3x3],
            &["R U".to_owned(), " F2 ".to_owned()],
        );
        assert_eq!(text, "1. 2x2x2: R U\n2. 3x3x3: F2\n");
    }
}
