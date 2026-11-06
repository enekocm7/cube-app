//! A drill: the selected cases of a set, dealt one scramble at a time.

use anyhow::{Result, bail};
use tnoodle::java::JavaRandom;

use super::{AlgCase, AlgSet, Deck, generate_scramble};

/// The case waiting to be solved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    /// The position of the case in the set.
    pub case: usize,
    pub scramble: String,
}

/// Works through the selected cases in random order, all of them before any
/// comes back, then again in a new order.
#[derive(Debug)]
pub struct Drill {
    set: &'static AlgSet,
    deck: Deck<usize>,
    prompt: Prompt,
    random: JavaRandom,
}

impl Drill {
    /// Starts drilling the cases of `set` at the positions in `cases`.
    pub fn new(set: &'static AlgSet, cases: Vec<usize>) -> Result<Self> {
        Self::with_random(set, cases, JavaRandom::from_entropy())
    }

    fn with_random(
        set: &'static AlgSet,
        cases: Vec<usize>,
        mut random: JavaRandom,
    ) -> Result<Self> {
        let mut deck = Deck::new(cases);
        let Some(case) = deck.draw(&mut random) else {
            bail!("No cases are selected");
        };
        let scramble = generate_scramble(set, &set.cases[case], &mut random)?;
        Ok(Self {
            set,
            deck,
            prompt: Prompt { case, scramble },
            random,
        })
    }

    pub const fn set(&self) -> &'static AlgSet {
        self.set
    }

    pub const fn prompt(&self) -> &Prompt {
        &self.prompt
    }

    /// Returns the case waiting to be solved.
    pub fn case(&self) -> &'static AlgCase {
        &self.set.cases[self.prompt.case]
    }

    /// Returns which case of this pass is showing and how many a pass has.
    pub const fn progress(&self) -> (usize, usize) {
        (self.deck.dealt(), self.deck.len())
    }

    /// Moves on to the next case.
    pub fn advance(&mut self) -> Result<()> {
        let Some(case) = self.deck.draw(&mut self.random) else {
            bail!("No cases are selected");
        };
        let scramble = generate_scramble(self.set, &self.set.cases[case], &mut self.random)?;
        self.prompt = Prompt { case, scramble };
        Ok(())
    }

    /// Moves on without having trained the case, which comes back before the
    /// pass ends.
    pub fn skip(&mut self) -> Result<()> {
        self.deck.defer();
        self.advance()
    }
}

#[cfg(test)]
mod tests {
    use super::super::PLL;
    use super::*;

    fn drill(cases: Vec<usize>) -> Drill {
        Drill::with_random(&PLL, cases, JavaRandom::new(21)).unwrap()
    }

    #[test]
    fn nothing_selected_is_an_error() {
        assert!(Drill::new(&PLL, Vec::new()).is_err());
    }

    #[test]
    fn every_case_comes_up_before_any_comes_back() {
        let mut drill = drill(vec![0, 4, 8, 12]);
        for _ in 0..5 {
            let mut seen = vec![drill.prompt().case];
            for _ in 0..3 {
                drill.advance().unwrap();
                seen.push(drill.prompt().case);
            }
            drill.advance().unwrap();
            seen.sort_unstable();
            assert_eq!(seen, [0, 4, 8, 12]);
        }
    }

    #[test]
    fn progress_counts_through_the_pass() {
        let mut drill = drill(vec![0, 1, 2]);
        assert_eq!(drill.progress(), (1, 3));
        drill.advance().unwrap();
        assert_eq!(drill.progress(), (2, 3));
        drill.advance().unwrap();
        drill.advance().unwrap();
        assert_eq!(drill.progress(), (1, 3));
    }

    #[test]
    fn a_skipped_case_is_still_trained_in_the_pass() {
        let mut drill = drill(vec![0, 1, 2, 3]);
        let skipped = drill.prompt().case;
        drill.skip().unwrap();
        assert_ne!(drill.prompt().case, skipped);
        assert_eq!(drill.progress(), (1, 4));
        drill.advance().unwrap();
        drill.advance().unwrap();
        drill.advance().unwrap();
        assert_eq!(drill.prompt().case, skipped);
    }

    #[test]
    fn every_case_gets_a_scramble() {
        let mut drill = drill(vec![0]);
        let first = drill.prompt().scramble.clone();
        assert_ne!(first, "");
        drill.advance().unwrap();
        assert_eq!(drill.case().name, PLL.cases[0].name);
    }
}
