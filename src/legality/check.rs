//! Check detection — the foundation of legality.
//!
//! Two primitives, deliberately isolated because reused by several consumers:
//! the self-check filter ([`crate::legality::self_check`]), checkmate
//! ([`crate::terminal`]), castling legality (squares traversed) and *uchifuzume*
//! (mate by foot-soldier drop).
//!
//! The module is **decoupled from `Position`**: it operates on a `piece_at`
//! closure and the opponent's variant, which makes it pure and testable.

use crate::domain::piece::Piece;
use crate::domain::side::Side;
use crate::domain::square::Square;
use crate::domain::variant::Variant;
use crate::movement::attack::is_attacked;

/// Locates the square of side `side`'s royal, if it is present on the board.
///
/// A well-formed position has exactly one royal per side; we return the first
/// occurrence in [`Square::all`] order. `None` signals the absence of a royal for
/// that side — a case the caller interprets according to context (malformed
/// position).
#[inline]
#[must_use]
pub fn royal_square(side: Side, piece_at: impl Fn(Square) -> Option<Piece>) -> Option<Square> {
    Square::all().find(|&square| match piece_at(square) {
        Some(piece) => piece.is_royal() && piece.belongs_to(side),
        None => false,
    })
}

/// True if side `side`'s royal is in check: its square is attacked by at least
/// one enemy piece.
///
/// `opponent_variant` is the opposing side's (the attacker's) variant. With no
/// royal for `side`, returns `false`: there is no royal to put in check. Callers
/// that must distinguish this case (malformed position) use [`royal_square`]
/// first.
#[inline]
#[must_use]
pub fn in_check(
    side: Side,
    opponent_variant: Variant,
    piece_at: impl Fn(Square) -> Option<Piece>,
) -> bool {
    match royal_square(side, &piece_at) {
        Some(royal) => is_attacked(royal, side.flip(), opponent_variant, &piece_at),
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

    use super::{in_check, royal_square};
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
    fn royal_locates_the_right_side() {
        let b = board(&[("e1", "K^"), ("e8", "k^")]);
        assert_eq!(royal_square(Side::First, &b), Some(sq("e1")));
        assert_eq!(royal_square(Side::Second, &b), Some(sq("e8")));
    }

    #[test]
    fn royal_absent() {
        let b = board(&[("e1", "K^")]);
        assert_eq!(royal_square(Side::Second, &b), None);
        // No royal -> not in check.
        assert!(!in_check(Side::Second, Variant::Chess, &b));
    }

    #[test]
    fn check_by_rook_on_open_line() {
        let b = board(&[("e1", "K^"), ("e8", "r")]);
        assert!(in_check(Side::First, Variant::Chess, &b));
        // Interposition: the check disappears.
        let b2 = board(&[("e1", "K^"), ("e8", "r"), ("e4", "P")]);
        assert!(!in_check(Side::First, Variant::Chess, &b2));
    }

    #[test]
    fn check_by_knight() {
        let b = board(&[("e1", "K^"), ("f3", "n")]);
        assert!(in_check(Side::First, Variant::Chess, &b));
    }

    #[test]
    fn check_by_pawn_diagonally() {
        // Enemy pawn (Second) on d2: it attacks c1 and e1 (forward = -rank).
        let b = board(&[("e1", "K^"), ("d2", "p")]);
        assert!(in_check(Side::First, Variant::Chess, &b));
        // An enemy pawn straight ahead (e2) does NOT give check (diagonal capture).
        let b2 = board(&[("e1", "K^"), ("e2", "p")]);
        assert!(!in_check(Side::First, Variant::Chess, &b2));
    }

    #[test]
    fn flying_general_xiongqi() {
        // Two Generals on the open e-file: mutual check at Chariot range.
        let b = board(&[("e1", "G^"), ("e8", "g^")]);
        assert!(in_check(Side::First, Variant::Xiongqi, &b));
        assert!(in_check(Side::Second, Variant::Xiongqi, &b));
        // An interposed piece cuts the line. We pick a Knight: it blocks the
        // slide without itself threatening the file (a Rook, by contrast, would
        // give check from e5).
        let b2 = board(&[("e1", "G^"), ("e8", "g^"), ("e5", "n")]);
        assert!(!in_check(Side::First, Variant::Xiongqi, &b2));
    }
}
