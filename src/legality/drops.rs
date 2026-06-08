//! **Structural** drop legality — specific to **ōgi** (neither chess nor xiongqi
//! has a droppable hand; in cross-variant play, only the ōgi player drops, and
//! always ōgi pieces).
//!
//! Every drop requires an **empty destination square** (a drop does not capture).
//! Three additional restrictions target only the **Fu** (`F`/`f`):
//!
//! 1. **No Fu on its own last rank** — there would be no legal move (the Fu does
//!    not move backward).
//! 2. **Nifu (二歩)** — no second **unpromoted** Fu on a file where the player
//!    already has an unpromoted Fu (a Tokin `T` does not count).
//! 3. **Uchifuzume (打ち歩詰め)** — dropping a Fu to deliver mate is forbidden.
//!
//! This module covers **1, 2 and the empty square**. **Uchifuzume** (3) requires
//! checkmate detection (hence enumerating the opponent's legal moves after the
//! drop); it will be composed separately by the kernel once `terminal` is
//! available. A drop's self-check is already covered by
//! [`crate::legality::self_check`]. The other droppable pieces (Rook, Bishop,
//! Knight, Princess) have no restriction beyond the empty square: on an 8×8 board
//! they always have a move.
//!
//! **Hand availability** (the piece is indeed in the player's hand) is the
//! kernel's concern, which reads `Position`'s hand; this module judges only the
//! placement on the board, via a `piece_at` closure.

use crate::domain::piece::Piece;
use crate::domain::side::Side;
use crate::domain::square::Square;
use core::fmt;

/// Index of the first player's last rank (rank 8).
const LAST_RANK: u8 = Square::RANK_COUNT - 1;

/// Checks the structural legality of dropping `piece` on `to` (empty square, and
/// — for a Fu — last rank and nifu).
///
/// **Does not cover uchifuzume** (cf. module header) nor self-check.
///
/// # Errors
/// Returns the [`DropError`] describing the first structural violation found.
pub fn drop_is_legal(
    piece: Piece,
    to: Square,
    piece_at: impl Fn(Square) -> Option<Piece>,
) -> Result<(), DropError> {
    // A drop does not capture: the square must be empty.
    if piece_at(to).is_some() {
        return Err(DropError::OccupiedDestination);
    }

    // Fu-specific restrictions.
    if piece.kind_letter() == 'F' {
        let side = piece.side();
        if to.rank() == last_rank(side) {
            return Err(DropError::FuOnLastRank);
        }
        if has_unpromoted_fu(side, to.file(), &piece_at) {
            return Err(DropError::Nifu);
        }
    }

    Ok(())
}

/// True if side `side` already has an **unpromoted** Fu (`F`/`f`, excluding the
/// Tokin `T`) on file `file`.
fn has_unpromoted_fu(side: Side, file: u8, piece_at: &impl Fn(Square) -> Option<Piece>) -> bool {
    Square::all()
        .filter(|square| square.file() == file)
        .filter_map(&piece_at)
        .any(|piece| piece.kind_letter() == 'F' && piece.belongs_to(side))
}

/// Side `side`'s last rank (the promotion rank).
#[inline]
const fn last_rank(side: Side) -> u8 {
    match side {
        Side::First => LAST_RANK,
        Side::Second => 0,
    }
}

/// The cause of a drop's **structural** illegality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropError {
    /// The destination square is occupied (a drop does not capture).
    OccupiedDestination,
    /// A Fu would be dropped on its own last rank (with no subsequent move).
    FuOnLastRank,
    /// Nifu: an unpromoted Fu of the same side is already on this file.
    Nifu,
}

impl fmt::Display for DropError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::OccupiedDestination => "occupied destination square: a drop does not capture",
            Self::FuOnLastRank => "Fu dropped on its own last rank",
            Self::Nifu => "nifu: an unpromoted Fu of the side is already on this file",
        };
        f.write_str(message)
    }
}

impl core::error::Error for DropError {}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::{drop_is_legal, DropError};
    use crate::domain::piece::Piece;
    use crate::domain::square::Square;
    use sashite_epin::Identifier as Epin;

    fn sq(s: &str) -> Square {
        Square::parse(s).expect("valid square")
    }

    fn piece(token: &str) -> Piece {
        Piece::new(Epin::parse(token).expect("valid EPIN"))
    }

    fn board<'a>(squares: &'a [(&'a str, &'a str)]) -> impl Fn(Square) -> Option<Piece> + 'a {
        move |s| {
            squares
                .iter()
                .find(|(name, _)| sq(name) == s)
                .map(|(_, tok)| piece(tok))
        }
    }

    fn empty(_: Square) -> Option<Piece> {
        None
    }

    #[test]
    fn drop_on_empty_square_legal() {
        assert_eq!(drop_is_legal(piece("R"), sq("e4"), empty), Ok(()));
        assert_eq!(drop_is_legal(piece("F"), sq("e4"), empty), Ok(()));
    }

    #[test]
    fn drop_on_occupied_square_refused() {
        let b = board(&[("e4", "n")]);
        assert_eq!(
            drop_is_legal(piece("F"), sq("e4"), &b),
            Err(DropError::OccupiedDestination)
        );
    }

    #[test]
    fn fu_on_last_rank_refused() {
        // First player's Fu on rank 8.
        assert_eq!(
            drop_is_legal(piece("F"), sq("e8"), empty),
            Err(DropError::FuOnLastRank)
        );
        // Second player's Fu on rank 1.
        assert_eq!(
            drop_is_legal(piece("f"), sq("e1"), empty),
            Err(DropError::FuOnLastRank)
        );
    }

    #[test]
    fn nifu_refused() {
        // The first player already has an unpromoted Fu on e3: dropping a Fu on e5 is illegal.
        let b = board(&[("e3", "F")]);
        assert_eq!(
            drop_is_legal(piece("F"), sq("e5"), &b),
            Err(DropError::Nifu)
        );
    }

    #[test]
    fn tokin_does_not_trigger_nifu() {
        // A Tokin (promoted Fu) on the file does not count for nifu.
        let b = board(&[("e3", "T")]);
        assert_eq!(drop_is_legal(piece("F"), sq("e5"), &b), Ok(()));
    }

    #[test]
    fn enemy_fu_does_not_trigger_nifu() {
        // Only same-side Fu count.
        let b = board(&[("e3", "f")]);
        assert_eq!(drop_is_legal(piece("F"), sq("e5"), &b), Ok(()));
    }

    #[test]
    fn nifu_is_per_file_not_global() {
        // One's own Fu on e3 does not prevent a drop on another file.
        let b = board(&[("e3", "F")]);
        assert_eq!(drop_is_legal(piece("F"), sq("d5"), &b), Ok(()));
    }

    #[test]
    fn non_fu_piece_without_rank_restriction() {
        // A Knight may be dropped on the last rank.
        assert_eq!(drop_is_legal(piece("N"), sq("e8"), empty), Ok(()));
    }
}
