//! The **100-half-move** rule (the FIDE 50-move rule, applied identically to the
//! three variants).
//!
//! If **50 full moves by each player** (100 half-moves) elapse without a capture
//! and without an **unpromoted foot-soldier** move (Pawn / Fu / Soldier), the game
//! is **automatically** a draw (`movelimit`) — no claim is involved. The half-move
//! counter is reset by any capture or any board move of an unpromoted foot
//! soldier (cf. `backend-logic.md`).
//!
//! A **drop** does not reset the counter: it is not a *move* of a foot soldier
//! already on the board, and it does not capture. Callers therefore pass `None`
//! as the moving piece for a drop. The counter itself is maintained over the game
//! by the kernel/arbiter; this module provides the reset rule, the saturating
//! advance and the threshold.

use crate::domain::piece::Piece;

/// Number of half-moves at which the game is automatically drawn.
pub const HALF_MOVE_LIMIT: u32 = 100;

/// True if this half-move resets the counter: a **capture**, or a board move of an
/// **unpromoted foot soldier**.
///
/// `board_mover` is the piece that moved **on the board** (its identity before any
/// promotion), or `None` for a **drop**. `captured` indicates whether the
/// half-move captured a piece.
#[inline]
#[must_use]
pub fn clock_resets(board_mover: Option<Piece>, captured: bool) -> bool {
    captured || matches!(board_mover, Some(piece) if piece.is_foot_soldier())
}

/// Half-move counter **after** this half-move: 0 on a reset, otherwise the old
/// value incremented (saturating).
#[inline]
#[must_use]
pub fn advanced_clock(current: u32, board_mover: Option<Piece>, captured: bool) -> u32 {
    if clock_resets(board_mover, captured) {
        0
    } else {
        current.saturating_add(1)
    }
}

/// True if the counter reaches the limit — the `movelimit` draw is available.
#[inline]
#[must_use]
pub const fn limit_reached(half_moves: u32) -> bool {
    half_moves >= HALF_MOVE_LIMIT
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::{advanced_clock, clock_resets, limit_reached, HALF_MOVE_LIMIT};
    use crate::domain::piece::Piece;
    use sashite_epin::Identifier as Epin;

    fn piece(token: &str) -> Piece {
        Piece::new(Epin::parse(token).expect("valid EPIN"))
    }

    #[test]
    fn capture_resets() {
        // Capture by any piece (here a Rook): reset.
        assert!(clock_resets(Some(piece("R")), true));
        assert_eq!(advanced_clock(42, Some(piece("R")), true), 0);
    }

    #[test]
    fn foot_soldier_move_resets() {
        for token in ["P", "f", "S"] {
            assert!(clock_resets(Some(piece(token)), false), "{token}");
        }
        assert_eq!(advanced_clock(42, Some(piece("P")), false), 0);
    }

    #[test]
    fn non_foot_soldier_move_without_capture_increments() {
        assert!(!clock_resets(Some(piece("R")), false));
        assert_eq!(advanced_clock(42, Some(piece("R")), false), 43);
        // The Tokin is not a foot soldier: its move does not reset.
        assert!(!clock_resets(Some(piece("T")), false));
        assert_eq!(advanced_clock(7, Some(piece("t")), false), 8);
    }

    #[test]
    fn drop_increments_without_reset() {
        // Drop: `None`, never a capture -> increments.
        assert!(!clock_resets(None, false));
        assert_eq!(advanced_clock(42, None, false), 43);
    }

    #[test]
    fn threshold() {
        assert!(!limit_reached(99));
        assert!(limit_reached(HALF_MOVE_LIMIT));
        assert!(limit_reached(101));
    }

    #[test]
    fn saturating_advance() {
        assert_eq!(advanced_clock(u32::MAX, Some(piece("R")), false), u32::MAX);
    }
}
