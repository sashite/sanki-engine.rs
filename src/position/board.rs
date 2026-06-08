//! Mapping between a Sanki [`Square`] and the **flat index** of a
//! [`Qi`](sashite_qi::Qi) board.
//!
//! `Qi` indexes its board in **row-major** order, the **top** rank first
//! (index 0 = top-left corner), which matches FEEN's enumeration order (which
//! lists rank 8 first). Our [`Square`], by contrast, has rank 0 at the bottom
//! (rank `1`). The conversion is therefore a simple **vertical flip**:
//!
//! ```text
//! index = (7 − rank) × 8 + file          (rank, file ∈ 0..=7)
//! ```
//!
//! hence `a8` (top-left) → 0 and `h1` (bottom-right) → 63.
//!
//! All arithmetic is *saturating* / *checked*: bounded by `Square`'s invariant
//! (indices 0..=7), it never actually saturates or fails in practice, yet stays
//! free of `+ - * / %` (compliant with `deny(arithmetic_side_effects)`) and
//! panic-free.

use crate::domain::square::Square;

/// Number of files on the 8×8 board.
const FILES_U8: u8 = Square::FILE_COUNT;
/// Index of the top rank, `Square`-side (rank `8`).
const TOP_RANK_U8: u8 = Square::RANK_COUNT - 1;
/// Number of files, as `usize`.
const FILES: usize = Square::FILE_COUNT as usize;
/// Total number of squares.
const SQUARE_COUNT: usize = (Square::FILE_COUNT as usize) * (Square::RANK_COUNT as usize);

/// The flat `Qi` index of a square.
#[inline]
#[must_use]
pub fn to_index(square: Square) -> usize {
    // rank, file ∈ 0..=7: no actual saturation.
    let qi_rank = TOP_RANK_U8.saturating_sub(square.rank());
    let index = qi_rank
        .saturating_mul(FILES_U8)
        .saturating_add(square.file());
    usize::from(index)
}

/// The square corresponding to a flat `Qi` index, or `None` if the index is
/// outside the 8×8 board.
#[inline]
#[must_use]
pub fn from_index(index: usize) -> Option<Square> {
    if index >= SQUARE_COUNT {
        return None;
    }
    let qi_rank = index.checked_div(FILES)?;
    let file = index.checked_rem(FILES)?;
    // qi_rank ∈ 0..=7 (index < 64): the flip stays within 0..=7.
    let rank = usize::from(TOP_RANK_U8).checked_sub(qi_rank)?;
    Square::new(u8::try_from(file).ok()?, u8::try_from(rank).ok()?)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::{from_index, to_index, SQUARE_COUNT};
    use crate::domain::square::Square;

    fn sq(s: &str) -> Square {
        Square::parse(s).expect("valid square")
    }

    #[test]
    fn corners_and_center() {
        assert_eq!(to_index(sq("a8")), 0); // top-left
        assert_eq!(to_index(sq("h8")), 7); // top-right
        assert_eq!(to_index(sq("a1")), 56); // bottom-left
        assert_eq!(to_index(sq("h1")), 63); // bottom-right
        assert_eq!(to_index(sq("e4")), 36);
    }

    #[test]
    fn from_index_inverts_corners() {
        assert_eq!(from_index(0), Some(sq("a8")));
        assert_eq!(from_index(7), Some(sq("h8")));
        assert_eq!(from_index(56), Some(sq("a1")));
        assert_eq!(from_index(63), Some(sq("h1")));
        assert_eq!(from_index(36), Some(sq("e4")));
    }

    #[test]
    fn from_index_off_board() {
        assert_eq!(from_index(SQUARE_COUNT), None);
        assert_eq!(from_index(1000), None);
    }

    #[test]
    fn to_index_is_a_bijection_over_0_63() {
        let mut indices: Vec<usize> = Square::all().map(to_index).collect();
        indices.sort_unstable();
        let expected: Vec<usize> = (0..SQUARE_COUNT).collect();
        assert_eq!(indices, expected, "to_index must cover exactly 0..64");
    }

    #[test]
    fn round_trip_square_index_square() {
        for s in Square::all() {
            assert_eq!(from_index(to_index(s)), Some(s), "round-trip {s}");
        }
    }

    #[test]
    fn round_trip_index_square_index() {
        for i in 0..SQUARE_COUNT {
            let s = from_index(i).expect("index on the board");
            assert_eq!(to_index(s), i, "round-trip index {i}");
        }
    }
}
