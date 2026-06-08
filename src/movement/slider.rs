//! Sliders: pure geometry of pieces that slide along directions, without
//! leaping, stopping at the first occupied square.
//!
//! Used by the Rook / Chariot's move (orthogonal), the Bishop / Bear (diagonal),
//! the Queen (all eight), the Empress's sliding component (orthogonal), and the
//! General's capture (orthogonal).
//!
//! Two operations, both parameterized by an [`Occupancy`] view:
//! - [`destinations`]: the squares reachable as a **move** (empty squares
//!   traversed + the first enemy square for the capture);
//! - [`attacks`]: whether the `target` square is **attacked** (every square
//!   strictly between the two being empty) — useful for check detection.

use crate::domain::square::Square;
use crate::movement::Occupancy;

/// The four orthogonal directions (deltas `(file, rank)`).
pub const ORTHOGONAL: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// The four diagonal directions.
pub const DIAGONAL: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

/// The eight directions (orthogonal + diagonal) — the Queen.
pub const OCTILINEAR: [(i8, i8); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

/// Squares reachable as a move by a slider from `from`, along `directions`: each
/// ray traverses the empty squares then stops at the first occupied square
/// (included only if it is an enemy — capture).
#[must_use]
pub fn destinations(
    from: Square,
    directions: &[(i8, i8)],
    occupancy: impl Fn(Square) -> Occupancy,
) -> Vec<Square> {
    let mut out = Vec::new();
    for &(df, dr) in directions {
        let mut sq = from;
        while let Some(next) = sq.offset(df, dr) {
            match occupancy(next) {
                Occupancy::Empty => {
                    out.push(next);
                    sq = next;
                }
                Occupancy::Enemy => {
                    out.push(next);
                    break;
                }
                Occupancy::Friendly => break,
            }
        }
    }
    out
}

/// True if a slider from `from`, along `directions`, **attacks** the `target`
/// square: `target` is reached before any occupied square on one of the rays.
/// (The occupant of `target` itself is not tested: a piece on `target` is
/// attacked, whatever its side.)
#[must_use]
pub fn attacks(
    from: Square,
    directions: &[(i8, i8)],
    target: Square,
    occupancy: impl Fn(Square) -> Occupancy,
) -> bool {
    for &(df, dr) in directions {
        let mut sq = from;
        while let Some(next) = sq.offset(df, dr) {
            if next == target {
                return true;
            }
            if occupancy(next).is_occupied() {
                break;
            }
            sq = next;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::{attacks, destinations, DIAGONAL, ORTHOGONAL};
    use crate::domain::square::Square;
    use crate::movement::Occupancy;

    fn sq(s: &str) -> Square {
        Square::parse(s).expect("valid square")
    }

    /// Occupancy view from an explicit list; everything else is empty.
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
    fn rook_empty_board() {
        let d = destinations(sq("d4"), &ORTHOGONAL, empty);
        assert_eq!(d.len(), 14, "7 on the file + 7 on the rank");
        assert!(d.contains(&sq("d8")));
        assert!(d.contains(&sq("d1")));
        assert!(d.contains(&sq("a4")));
        assert!(d.contains(&sq("h4")));
        assert!(!d.contains(&sq("d4")), "never its own square");
        assert!(!d.contains(&sq("e5")), "not diagonally");
    }

    #[test]
    fn bishop_empty_board() {
        let d = destinations(sq("d4"), &DIAGONAL, empty);
        assert_eq!(d.len(), 13);
        assert!(d.contains(&sq("a1")));
        assert!(d.contains(&sq("h8")));
        assert!(d.contains(&sq("a7")));
        assert!(d.contains(&sq("g1")));
        assert!(!d.contains(&sq("d5")), "not on a straight line");
    }

    #[test]
    fn rook_blocked_by_friendly_and_enemy_capture() {
        let board = occ(&[("d6", Occupancy::Friendly), ("f4", Occupancy::Enemy)]);
        let d = destinations(sq("d4"), &ORTHOGONAL, board);
        // North: d5 only (friendly d6 blocks, not included).
        assert!(d.contains(&sq("d5")));
        assert!(!d.contains(&sq("d6")));
        assert!(!d.contains(&sq("d7")));
        // East: e4 then capturing f4 (included), no further.
        assert!(d.contains(&sq("e4")));
        assert!(d.contains(&sq("f4")));
        assert!(!d.contains(&sq("g4")));
        // South and West free to the edge.
        assert!(d.contains(&sq("d1")));
        assert!(d.contains(&sq("a4")));
        assert_eq!(d.len(), 1 + 3 + 2 + 3);
    }

    #[test]
    fn direct_attack_empty_board() {
        assert!(attacks(sq("d4"), &ORTHOGONAL, sq("d8"), empty));
        assert!(attacks(sq("d4"), &ORTHOGONAL, sq("a4"), empty));
        assert!(attacks(sq("d4"), &DIAGONAL, sq("h8"), empty));
        assert!(
            !attacks(sq("d4"), &ORTHOGONAL, sq("e5"), empty),
            "off the ray"
        );
    }

    #[test]
    fn attack_blocked_by_intermediate_piece() {
        let board = occ(&[("d6", Occupancy::Enemy)]);
        // The occupied piece itself is attacked…
        assert!(attacks(sq("d4"), &ORTHOGONAL, sq("d6"), &board));
        // …but not the square behind.
        assert!(!attacks(sq("d4"), &ORTHOGONAL, sq("d8"), &board));
    }

    #[test]
    fn attack_reaches_an_empty_square_past_nothing() {
        // The target can be empty (useful for "does the king move into check?").
        let board = occ(&[("f6", Occupancy::Friendly)]);
        assert!(attacks(sq("d4"), &DIAGONAL, sq("e5"), &board));
        assert!(!attacks(sq("d4"), &DIAGONAL, sq("h8"), &board), "f6 blocks");
    }
}
