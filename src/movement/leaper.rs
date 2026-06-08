//! Leapers: pure geometry of pieces that jump to fixed-offset squares, **without
//! blocking** (they leap over intermediate pieces).
//!
//! Used by the Knight (chess, ōgi) and the "knight" component of the Princess and
//! the Empress (combined with a slider in the dispatch).
//!
//! Since a leaper cannot be blocked, [`attacks`] is purely geometric: it needs no
//! occupancy view (only the destination square matters). [`destinations`] needs
//! it only to exclude squares occupied by a friendly piece.

use crate::domain::square::Square;
use crate::movement::Occupancy;

/// The eight L-shaped knight leaps (deltas `(file, rank)`).
pub const KNIGHT: [(i8, i8); 8] = [
    (1, 2),
    (2, 1),
    (2, -1),
    (1, -2),
    (-1, -2),
    (-2, -1),
    (-2, 1),
    (-1, 2),
];

/// Squares reachable as a move by a leaper from `from`, via `offsets`: each
/// on-board destination square not occupied by a friendly piece (an enemy square
/// is a capture).
#[must_use]
pub fn destinations(
    from: Square,
    offsets: &[(i8, i8)],
    occupancy: impl Fn(Square) -> Occupancy,
) -> Vec<Square> {
    let mut out = Vec::new();
    for &(df, dr) in offsets {
        if let Some(target) = from.offset(df, dr) {
            if !matches!(occupancy(target), Occupancy::Friendly) {
                out.push(target);
            }
        }
    }
    out
}

/// True if a leaper from `from`, via `offsets`, **attacks** `target` — the
/// destination square of one of its leaps (occupancy has no effect: it leaps).
#[inline]
#[must_use]
pub fn attacks(from: Square, offsets: &[(i8, i8)], target: Square) -> bool {
    offsets
        .iter()
        .any(|&(df, dr)| from.offset(df, dr) == Some(target))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::{attacks, destinations, KNIGHT};
    use crate::domain::square::Square;
    use crate::movement::Occupancy;

    fn sq(s: &str) -> Square {
        Square::parse(s).expect("valid square")
    }

    fn occ<'a>(pieces: &'a [(&'a str, Occupancy)]) -> impl Fn(Square) -> Occupancy + 'a {
        move |s| {
            pieces
                .iter()
                .find(|(name, _)| sq(name) == s)
                .map_or(Occupancy::Empty, |(_, o)| *o)
        }
    }

    fn empty(_: Square) -> Occupancy {
        Occupancy::Empty
    }

    #[test]
    fn knight_in_center() {
        let d = destinations(sq("d4"), &KNIGHT, empty);
        assert_eq!(d.len(), 8);
        for target in ["e6", "f5", "f3", "e2", "c2", "b3", "b5", "c6"] {
            assert!(d.contains(&sq(target)), "must reach {target}");
        }
        assert!(!d.contains(&sq("d5")), "not a knight leap");
    }

    #[test]
    fn knight_in_corner() {
        // From a1, only two leaps stay on the board.
        let d = destinations(sq("a1"), &KNIGHT, empty);
        assert_eq!(d.len(), 2);
        assert!(d.contains(&sq("b3")));
        assert!(d.contains(&sq("c2")));
    }

    #[test]
    fn friendly_blocks_destination_not_enemy() {
        let board = occ(&[("e6", Occupancy::Friendly), ("f5", Occupancy::Enemy)]);
        let d = destinations(sq("d4"), &KNIGHT, board);
        assert!(!d.contains(&sq("e6")), "friendly destination square");
        assert!(d.contains(&sq("f5")), "enemy capture");
        // The leap ignores any intermediate piece; the other squares remain.
        assert_eq!(d.len(), 7);
    }

    #[test]
    fn attack_ignores_occupancy() {
        assert!(attacks(sq("d4"), &KNIGHT, sq("e6")));
        assert!(attacks(sq("d4"), &KNIGHT, sq("f5")));
        assert!(!attacks(sq("d4"), &KNIGHT, sq("d5")));
        assert!(!attacks(sq("d4"), &KNIGHT, sq("d6")));
        // From a corner.
        assert!(attacks(sq("a1"), &KNIGHT, sq("b3")));
        assert!(!attacks(sq("a1"), &KNIGHT, sq("a3")));
    }
}
