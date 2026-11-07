//! The algorithm library: every puzzle's sets of cases, with a diagram and
//! algorithms for each, what the user has learned, and which cases are due for
//! review.
//!
//! The case model is the trainer's [`AlgSet`](crate::trainer::AlgSet), and
//! there is one copy of the data: the files read by [`catalog`], which the
//! trainer drills a part of. Everything saved here lives in `library.json`,
//! never in `times.json`.

mod catalog;
mod check;
mod data;
pub mod geometry;
pub mod model;
mod review;
#[cfg(test)]
mod tests;

pub use catalog::{
    LibraryPuzzle, catalog, find_set, first_visible_set, library_sets, search_cases,
    search_visible_cases, visible_catalog, visible_sets,
};
pub use check::{
    Recognition, case_origins, case_state, check, free_turns, recognition, solves_from,
};
pub use data::{CaseEntry, DueCase, LibraryData, Outcome, Progress, Status};
pub use review::{Grade, Review};
