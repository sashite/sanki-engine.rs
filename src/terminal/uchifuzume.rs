//! Uchifuzume (打ち歩詰め) — the ban on **delivering mate with a Fu drop** (ōgi).
//!
//! Giving **check** with a Fu drop is allowed; only **checkmate** is forbidden,
//! and only for an **unpromoted Fu** (`F`/`f`), the only Fu token that can be in
//! hand. The rule applies to both royals.
//!
//! It is the last legality rule, long deferred because it presupposes checkmate
//! detection. It composes here on top of the layer already built: it simulates the
//! drop, then checks whether the opponent is **in check and without any legal
//! move**. As it depends on [`crate::terminal::legal_set`] and
//! [`crate::legality::check`], it lives at the `terminal` level (a one-way
//! dependency `terminal → legality`).
//!
//! The kernel, when resolving a drop, combines structural legality
//! ([`crate::legality::drops`]), self-check ([`crate::legality::self_check`]) and
//! the negation of [`is_uchifuzume`].

use crate::domain::piece::Piece;
use crate::domain::square::Square;
use crate::domain::variant::VariantAssignment;
use crate::legality::check::in_check;
use crate::terminal::legal_set::has_legal_move;

/// True if dropping `dropped` on `to` constitutes an uchifuzume: a **Fu** drop
/// that puts the opponent in **checkmate**.
///
/// `opponent_hand` enumerates the opponent's pieces in hand (for their possible
/// escaping moves, drops included). Returns `false` for any token other than a Fu.
#[must_use]
pub fn is_uchifuzume(
    dropped: Piece,
    to: Square,
    variants: VariantAssignment,
    piece_at: impl Fn(Square) -> Option<Piece>,
    opponent_hand: &[Piece],
) -> bool {
    // The ban targets only the unpromoted Fu.
    if dropped.kind_letter() != 'F' {
        return false;
    }

    let dropper = dropped.side();
    let opponent = dropper.flip();
    let dropper_variant = variants.variant_of(dropper);

    // Board after the drop: the `to` square receives the Fu.
    let post = |square: Square| -> Option<Piece> {
        if square == to {
            Some(dropped)
        } else {
            piece_at(square)
        }
    };

    // Checkmate = opponent in check AND without any legal move.
    in_check(opponent, dropper_variant, post)
        && !has_legal_move(opponent, variants, post, opponent_hand)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::is_uchifuzume;
    use crate::domain::piece::Piece;
    use crate::domain::square::Square;
    use crate::domain::variant::{Variant, VariantAssignment};
    use sashite_epin::Identifier as Epin;

    fn sq(s: &str) -> Square {
        Square::parse(s).expect("valid square")
    }

    fn piece(token: &str) -> Piece {
        Piece::new(Epin::parse(token).expect("valid EPIN"))
    }

    fn board<'a>(cases: &'a [(&'a str, &'a str)]) -> impl Fn(Square) -> Option<Piece> + 'a {
        move |s| {
            cases
                .iter()
                .find(|(name, _)| sq(name) == s)
                .map(|(_, tok)| piece(tok))
        }
    }

    const OGI: VariantAssignment = VariantAssignment {
        first: Variant::Ogi,
        second: Variant::Ogi,
    };

    #[test]
    fn mating_fu_drop_is_uchifuzume() {
        // Black King h8. White Rook g1 (covers g7, g8). White Knight f6 (covers g8
        // and defends h7). Dropping a white Fu on h7 gives check (the Fu attacks
        // upward) with no escape: it is mate -> illegal.
        let b = board(&[("h8", "k^"), ("g1", "R"), ("f6", "N")]);
        assert!(is_uchifuzume(piece("F"), sq("h7"), OGI, &b, &[]));
    }

    #[test]
    fn check_without_mate_is_allowed() {
        // Without the knight, the Fu on h7 is undefended: the King captures it.
        // Check without mate -> allowed (no uchifuzume).
        let b = board(&[("h8", "k^"), ("g1", "R")]);
        assert!(!is_uchifuzume(piece("F"), sq("h7"), OGI, &b, &[]));
    }

    #[test]
    fn capturable_checker_is_not_mate() {
        // Same mating configuration, but a black Rook on a7 can capture the Fu on
        // h7 along the 7th rank: an escape -> no uchifuzume.
        let b = board(&[("h8", "k^"), ("g1", "R"), ("f6", "N"), ("a7", "r")]);
        assert!(!is_uchifuzume(piece("F"), sq("h7"), OGI, &b, &[]));
    }

    #[test]
    fn non_fu_drop_is_never_uchifuzume() {
        // The rule targets only the Fu: another token is never uchifuzume, even in
        // a mating configuration.
        let b = board(&[("h8", "k^"), ("g1", "R"), ("f6", "N")]);
        assert!(!is_uchifuzume(piece("R"), sq("h7"), OGI, &b, &[]));
        assert!(!is_uchifuzume(piece("N"), sq("g6"), OGI, &b, &[]));
    }
}
