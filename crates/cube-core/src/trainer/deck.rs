//! Deals every case once, in random order, before any case comes back.

use tnoodle::java::RandomSource;

/// How many times a pass is shuffled in search of one with no item twice in a
/// row, before settling for the last.
const SHUFFLES: usize = 200;

/// A shuffled pile of items. When it runs out it is shuffled again, so all of
/// them keep coming up equally often.
#[derive(Clone, Debug)]
pub struct Deck<T> {
    /// Every item; the ones from `dealt` on are still to come.
    items: Vec<T>,
    dealt: usize,
}

impl<T: Copy + PartialEq> Deck<T> {
    /// A deck of `items`. The first draw shuffles it.
    pub const fn new(items: Vec<T>) -> Self {
        let dealt = items.len();
        Self { items, dealt }
    }

    /// Returns how many items a pass deals.
    pub const fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns whether there is nothing to deal.
    pub const fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns how many items this pass has dealt so far.
    pub const fn dealt(&self) -> usize {
        self.dealt
    }

    /// Deals the next item, starting a new pass after the last one. A new pass
    /// never opens with the item that closed the old one.
    pub fn draw(&mut self, random: &mut dyn RandomSource) -> Option<T> {
        if self.items.is_empty() {
            return None;
        }
        if self.dealt == self.items.len() {
            let last = self.items.last().copied();
            // Shuffle again until no item repeats, if it can be done at all.
            let varied = self.items.iter().any(|item| *item != self.items[0]);
            for _ in 0..SHUFFLES {
                self.shuffle(random);
                if !varied || self.is_separate(last) {
                    break;
                }
            }
            self.dealt = 0;
        }
        let item = self.items[self.dealt];
        self.dealt += 1;
        Some(item)
    }

    /// Puts the item just dealt back at the end of this pass, so skipping it
    /// does not count as having trained it. The last item of a pass has
    /// nothing left to go behind, so it stays dealt.
    pub fn defer(&mut self) {
        if self.dealt > 0 && self.dealt < self.items.len() {
            self.dealt -= 1;
            self.items[self.dealt..].rotate_left(1);
        }
    }

    /// Returns whether no item follows an equal one, with `last` the item
    /// that closed the previous pass. An item may be in the deck several
    /// times, to come up more often.
    fn is_separate(&self, last: Option<T>) -> bool {
        last != self.items.first().copied() && self.items.windows(2).all(|pair| pair[0] != pair[1])
    }

    /// Shuffles every item with Fisher-Yates.
    fn shuffle(&mut self, random: &mut dyn RandomSource) {
        for index in (1..self.items.len()).rev() {
            let other = bounded(random, index + 1);
            self.items.swap(index, other);
        }
    }
}

/// A random number below `bound`.
pub(super) fn bounded(random: &mut dyn RandomSource, bound: usize) -> usize {
    random.next_int_bounded(i32::try_from(bound).unwrap_or(i32::MAX)) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use tnoodle::java::JavaRandom;

    fn pass(deck: &mut Deck<u32>, random: &mut JavaRandom) -> Vec<u32> {
        (0..deck.len())
            .map(|_| deck.draw(random).unwrap())
            .collect()
    }

    #[test]
    fn every_pass_deals_each_item_once() {
        let mut random = JavaRandom::new(1);
        let mut deck = Deck::new(vec![1, 2, 3, 4, 5, 6, 7]);
        for _ in 0..50 {
            let mut dealt = pass(&mut deck, &mut random);
            dealt.sort_unstable();
            assert_eq!(dealt, [1, 2, 3, 4, 5, 6, 7]);
        }
    }

    #[test]
    fn passes_come_in_different_orders() {
        let mut random = JavaRandom::new(2);
        let mut deck = Deck::new((0..20).collect());
        let first = pass(&mut deck, &mut random);
        let second = pass(&mut deck, &mut random);
        assert_ne!(first, second);
    }

    #[test]
    fn a_new_pass_does_not_repeat_the_last_item() {
        let mut random = JavaRandom::new(3);
        let mut deck = Deck::new(vec![1, 2, 3]);
        let mut previous = None;
        for _ in 0..600 {
            let item = deck.draw(&mut random);
            assert_ne!(item, previous);
            previous = item;
        }
    }

    #[test]
    fn a_single_item_keeps_coming_up() {
        let mut random = JavaRandom::new(4);
        let mut deck = Deck::new(vec![9]);
        for _ in 0..3 {
            assert_eq!(deck.draw(&mut random), Some(9));
        }
    }

    #[test]
    fn an_empty_deck_deals_nothing() {
        let mut random = JavaRandom::new(5);
        assert_eq!(Deck::<u32>::new(Vec::new()).draw(&mut random), None);
    }

    #[test]
    fn a_deferred_item_comes_back_in_the_same_pass() {
        let mut random = JavaRandom::new(6);
        let mut deck = Deck::new(vec![1, 2, 3, 4]);
        let skipped = deck.draw(&mut random).unwrap();
        deck.defer();
        assert_eq!(deck.dealt(), 0);
        let mut rest = pass(&mut deck, &mut random);
        assert_eq!(rest.last(), Some(&skipped));
        rest.sort_unstable();
        assert_eq!(rest, [1, 2, 3, 4]);
    }

    #[test]
    fn deferring_the_last_item_of_a_pass_moves_on() {
        let mut random = JavaRandom::new(7);
        let mut deck = Deck::new(vec![1, 2]);
        pass(&mut deck, &mut random);
        deck.defer();
        assert_eq!(deck.dealt(), 2);
    }

    #[test]
    fn a_repeated_item_comes_up_that_often_but_not_twice_running() {
        let mut random = JavaRandom::new(8);
        let mut deck = Deck::new(vec![1, 1, 1, 2, 2, 3, 4]);
        let mut previous = None;
        for _ in 0..20 {
            let mut dealt = pass(&mut deck, &mut random);
            for item in &dealt {
                assert_ne!(Some(*item), previous);
                previous = Some(*item);
            }
            dealt.sort_unstable();
            assert_eq!(dealt, [1, 1, 1, 2, 2, 3, 4]);
        }
    }
}
