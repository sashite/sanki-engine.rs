//! FIDE self-check filter: a move is illegal if it **leaves or places** the
//! player's own royal in check. This is the filter common to all variants — it
//! converts pseudo-legality (geometry + blocking) into legality for simple
//! moves, captures, en passant and drops.
//!
//! The module is **decoupled from `Position` and `Qi`**: it resolves the move
//! into a minimal board effect, applied on the fly over the `piece_at` closure,
//! without materializing the next position.

use crate::domain::piece::Piece;
use crate::domain::side::Side;
use crate::domain::square::Square;
use crate::domain::variant::Variant;
use crate::legality::check::in_check;

/// True if the described move leaves `mover`'s royal **out of check** (the move
/// is therefore *safe* with respect to self-check).
///
/// The move is resolved into a minimal board effect:
///
/// - `origin` is emptied — `None` for a drop, which vacates no square;
/// - `placed` is set on `to`; it is supplied **already transformed** by the
///   caller (promotion, or simply the moved piece / the royal), which keeps this
///   module agnostic about promotion logic;
/// - `ep_captured`, if present, removes the en-passant-captured piece (on a
///   square **distinct** from `to`).
///
/// `opponent_variant` is the opponent's (the potential attacker's) variant.
///
/// **Castling** is not handled here: it moves two pieces and forbids *passing
/// through* check; its legality belongs to [`crate::legality::castling`].
#[inline]
#[must_use]
pub fn move_is_safe(
    mover: Side,
    opponent_variant: Variant,
    origin: Option<Square>,
    to: Square,
    placed: Piece,
    ep_captured: Option<Square>,
    piece_at: impl Fn(Square) -> Option<Piece>,
) -> bool {
    // Resulting board, computed square by square. The three special squares
    // (`ep_captured`, `to`, `origin`) are pairwise distinct for a well-formed
    // move; the branch order is therefore immaterial.
    let post = |square: Square| -> Option<Piece> {
        if Some(square) == ep_captured {
            None
        } else if square == to {
            Some(placed)
        } else if Some(square) == origin {
            None
        } else {
            piece_at(square)
        }
    };
    !in_check(mover, opponent_variant, post)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::move_is_safe;
    use crate::domain::piece::Piece;
    use crate::domain::side::Side;
    use crate::domain::square::Square;
    use crate::domain::variant::Variant;
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

    #[test]
    fn absolute_pin_refused() {
        // White bishop on e4 pinned by the black rook on e8 in front of the king on e1.
        let b = board(&[("e1", "K^"), ("e4", "B"), ("e8", "r")]);
        // Moving it off the file exposes the king -> unsafe.
        assert!(!move_is_safe(
            Side::First,
            Variant::Chess,
            Some(sq("e4")),
            sq("c6"),
            piece("B"),
            None,
            &b,
        ));
    }

    #[test]
    fn king_steps_out_of_check() {
        // King on e1, check from the rook on e8 along the open file.
        let b = board(&[("e1", "K^"), ("e8", "r")]);
        // Leaving the file: safe.
        assert!(move_is_safe(
            Side::First,
            Variant::Chess,
            Some(sq("e1")),
            sq("d1"),
            piece("K^"),
            None,
            &b,
        ));
        // Staying on the file: still in check.
        assert!(!move_is_safe(
            Side::First,
            Variant::Chess,
            Some(sq("e1")),
            sq("e2"),
            piece("K^"),
            None,
            &b,
        ));
    }

    #[test]
    fn capture_of_the_checking_piece() {
        // Black rook on e2 giving check, undefended: the king captures it.
        let b = board(&[("e1", "K^"), ("e2", "r")]);
        assert!(move_is_safe(
            Side::First,
            Variant::Chess,
            Some(sq("e1")),
            sq("e2"),
            piece("K^"),
            None,
            &b,
        ));
    }

    #[test]
    fn en_passant_that_unpins_the_king() {
        // Classic case: king a5, pawn b5; black pawn c5 having just played the
        // double step; black rook h5 on the 5th rank. The en-passant capture
        // b5xc6 removes TWO pawns from the rank and exposes the king to the rook.
        let b = board(&[("a5", "K^"), ("b5", "P"), ("c5", "p"), ("h5", "r")]);
        assert!(!move_is_safe(
            Side::First,
            Variant::Chess,
            Some(sq("b5")),
            sq("c6"),
            piece("P"),
            Some(sq("c5")),
            &b,
        ));
        // Without the rook, the same capture is safe.
        let b2 = board(&[("a5", "K^"), ("b5", "P"), ("c5", "p")]);
        assert!(move_is_safe(
            Side::First,
            Variant::Chess,
            Some(sq("b5")),
            sq("c6"),
            piece("P"),
            Some(sq("c5")),
            &b2,
        ));
    }

    #[test]
    fn drop_must_parry_check() {
        // King e1 in check from the rook e8 (ōgi). A drop empties no square
        // (origin = None).
        let b = board(&[("e1", "K^"), ("e8", "r")]);
        // Drop off the file: does not parry -> unsafe.
        assert!(!move_is_safe(
            Side::First,
            Variant::Ogi,
            None,
            sq("b4"),
            piece("F"),
            None,
            &b,
        ));
        // Drop interposing on the file: safe.
        assert!(move_is_safe(
            Side::First,
            Variant::Ogi,
            None,
            sq("e4"),
            piece("F"),
            None,
            &b,
        ));
    }
}
