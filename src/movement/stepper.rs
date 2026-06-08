//! Step: pure geometry of pieces that move **one** square along given
//! directions (no sliding, hence no intermediate blocking).
//!
//! - the **King** (`K^`): one step in the eight directions, move or capture
//!   ([`destinations`] with `captures = true`, offsets [`KING`]);
//! - the **General**'s *non-capturing* step (`G^`): one step toward an **empty**
//!   square in the four orthogonals ([`destinations`] with `captures = false`).
//!   Its **capture** is Chariot-like and belongs to the slider; likewise its
//!   **attack** (the "flying general" file) is orthogonal-sliding — so it is not
//!   [`attacks`] here, which only describes the reach of a step.

use crate::domain::square::Square;
use crate::movement::Occupancy;

/// The eight adjacent squares (offsets `(file, rank)`) — the King's step.
/// Numerically identical to the eight directions, but here they are destination
/// squares at distance 1, not rays.
pub const KING: [(i8, i8); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

/// Squares reachable as a move by a step from `from`, along `directions`: each
/// adjacent on-board square that is **empty**, plus — if `captures` — enemy
/// squares (capture). Friendly squares are excluded.
#[must_use]
pub fn destinations(
    from: Square,
    directions: &[(i8, i8)],
    captures: bool,
    occupancy: impl Fn(Square) -> Occupancy,
) -> Vec<Square> {
    let mut out = Vec::new();
    for &(df, dr) in directions {
        if let Some(target) = from.offset(df, dr) {
            match occupancy(target) {
                Occupancy::Empty => out.push(target),
                Occupancy::Enemy => {
                    if captures {
                        out.push(target);
                    }
                }
                Occupancy::Friendly => {}
            }
        }
    }
    out
}

/// True if a step from `from`, along `directions`, **attacks** `target` (one of
/// its adjacent squares). Suits the King; the General's attack reach, being
/// Chariot-like, belongs to the slider instead.
#[inline]
#[must_use]
pub fn attacks(from: Square, directions: &[(i8, i8)], target: Square) -> bool {
    directions
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

    use super::{attacks, destinations, KING};
    use crate::domain::square::Square;
    use crate::movement::Occupancy;

    /// The four orthogonals — the General's step (passed by the caller).
    const ORTHOGONAL: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

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
    fn king_in_center() {
        let d = destinations(sq("d4"), &KING, true, empty);
        assert_eq!(d.len(), 8);
        for target in ["c3", "c4", "c5", "d3", "d5", "e3", "e4", "e5"] {
            assert!(d.contains(&sq(target)), "{target}");
        }
    }

    #[test]
    fn king_in_corner() {
        let d = destinations(sq("a1"), &KING, true, empty);
        assert_eq!(d.len(), 3);
        assert!(d.contains(&sq("a2")));
        assert!(d.contains(&sq("b1")));
        assert!(d.contains(&sq("b2")));
    }

    #[test]
    fn king_captures_enemy_not_friendly() {
        let board = occ(&[("e5", Occupancy::Enemy), ("d5", Occupancy::Friendly)]);
        let d = destinations(sq("d4"), &KING, true, board);
        assert!(d.contains(&sq("e5")), "capture");
        assert!(!d.contains(&sq("d5")), "friendly");
    }

    #[test]
    fn general_non_capturing_step() {
        // Empty behind, enemy ahead (d5), friendly to the left (c4).
        let board = occ(&[("d5", Occupancy::Enemy), ("c4", Occupancy::Friendly)]);
        let d = destinations(sq("d4"), &ORTHOGONAL, false, board);
        // Only the empty orthogonal squares: d3 and e4.
        assert!(d.contains(&sq("d3")));
        assert!(d.contains(&sq("e4")));
        assert!(!d.contains(&sq("d5")), "the step does not capture");
        assert!(!d.contains(&sq("c4")), "friendly");
        assert_eq!(d.len(), 2);
    }

    #[test]
    fn king_attack() {
        assert!(attacks(sq("d4"), &KING, sq("e5")));
        assert!(attacks(sq("d4"), &KING, sq("d5")));
        assert!(!attacks(sq("d4"), &KING, sq("d6")), "two squares");
        assert!(!attacks(sq("d4"), &KING, sq("f4")), "two squares");
    }
}
