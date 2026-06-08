//! Threefold repetition — `repetition` draw (the FIDE rule, identical across the
//! three variants).
//!
//! A game is drawn as soon as **the same position occurs three times** (not
//! necessarily consecutively). A position's identity is its **canonical FEEN**,
//! which encodes the board (including the `+P`/`-P`/`+R`/`-R`/`-K^` markers), both
//! players' **hands**, and the **styles + active player**: two FEEN-identical
//! positions are the same position.
//!
//! This module stays **representation-agnostic**: it operates on opaque,
//! comparable position keys (`K: PartialEq`, typically the FEEN `String`s produced
//! by canonicalization). The chronological history of positions is kept by the
//! arbiter over the game; this module counts occurrences and signals the threshold.

/// Number of occurrences of the same position that draws the game.
pub const THREEFOLD: usize = 3;

/// Number of occurrences of `key` among `positions`.
#[must_use]
pub fn occurrences<K: PartialEq>(positions: &[K], key: &K) -> usize {
    positions
        .iter()
        .filter(|candidate| **candidate == *key)
        .count()
}

/// True if the **latest** position in the history reaches the threefold-repetition
/// threshold.
///
/// `positions` is the chronological list of all positions reached (one key per
/// position, the most recent last). Since the draw is observed at the moment a
/// position occurs for the 3rd time, we test the current position (the last one):
/// called after every half-move over the game, this test captures the repetition
/// at the instant it happens.
#[inline]
#[must_use]
pub fn is_threefold<K: PartialEq>(positions: &[K]) -> bool {
    match positions.last() {
        Some(latest) => occurrences(positions, latest) >= THREEFOLD,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::{is_threefold, occurrences, THREEFOLD};

    #[test]
    fn counts_occurrences() {
        let positions = ["A", "B", "A", "C", "A"];
        assert_eq!(occurrences(&positions, &"A"), 3);
        assert_eq!(occurrences(&positions, &"B"), 1);
        assert_eq!(occurrences(&positions, &"Z"), 0);
    }

    #[test]
    fn third_occurrence_of_the_current() {
        // The latest position ("A") occurs for the 3rd time.
        let positions = ["A", "B", "A", "C", "A"];
        assert!(is_threefold(&positions));
    }

    #[test]
    fn two_occurrences_are_not_enough() {
        // The latest ("B") appears only twice.
        let positions = ["A", "B", "A", "C", "B"];
        assert!(!is_threefold(&positions));
    }

    #[test]
    fn tests_the_current_position_not_an_earlier_one() {
        // "A" tripled earlier, but the current position is "B" (1 occurrence): over
        // the game, "A" would have triggered the draw when it was current (index 2).
        // Here, no draw on the current position.
        let positions = ["A", "A", "A", "B"];
        assert!(!is_threefold(&positions));
        // And "A" alone, at the end of the list, would indeed trigger it.
        let at_third = ["A", "A", "A"];
        assert!(is_threefold(&at_third));
    }

    #[test]
    fn empty_or_short_history() {
        let empty: [&str; 0] = [];
        assert!(!is_threefold(&empty));
        assert!(!is_threefold(&["A"]));
        assert!(!is_threefold(&["A", "A"]));
    }

    #[test]
    fn threshold_is_three() {
        assert_eq!(THREEFOLD, 3);
    }

    #[test]
    fn works_with_strings() {
        // Owned keys (String), like canonical FEENs.
        let positions = vec![
            String::from("pos-1"),
            String::from("pos-2"),
            String::from("pos-1"),
            String::from("pos-1"),
        ];
        assert!(is_threefold(&positions));
    }
}
