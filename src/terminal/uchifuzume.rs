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
//! The kernel and the `engine` façade, when resolving a drop, combine
//! structural legality ([`crate::legality::drops`]), self-check
//! ([`crate::legality::self_check`]) and the negation of [`is_uchifuzume`].
//!
//! ## Single-square property (the fast gate)
//!
//! A drop adds a piece to the board: it can **block** existing attack lines but
//! can never **discover** a new one. On a coherent position — the dropper is to
//! move, so the opponent is *not* in check before the drop — a post-drop check
//! on the opponent can therefore only come from the dropped Fu itself, whose
//! sole attack is the square directly in front of it. An uchifuzume is thus
//! only possible when the opponent's **royal stands exactly one square forward
//! of `to`** (from the dropper's perspective): at most **one** square per
//! position can ever be an uchifuzume drop. [`is_uchifuzume`] encodes this as
//! its first test, which makes probing every candidate drop essentially free.
//!
//! ## Inner mate test and termination
//!
//! The mate test below uses the plain (resolve-level)
//! [`has_legal_move`] for the opponent's escapes — **not** the
//! uchifuzume-aware [`crate::terminal::legal_set::has_full_legal_move`] — so
//! the two functions cannot recurse. This is not an approximation: the
//! opponent, checked by the dropped Fu from the **adjacent** square, cannot
//! parry by interposition, so no *drop* of theirs can be an escape; their
//! escape set contains only royal moves and captures, on which the two
//! legality readings coincide.

use crate::domain::piece::Piece;
use crate::domain::square::Square;
use crate::domain::variant::VariantAssignment;
use crate::legality::check::in_check;
use crate::movement::forward;
use crate::position::Position;
use crate::terminal::legal_set::has_legal_move;

/// True if dropping `dropped` on `to` constitutes an uchifuzume: a **Fu** drop
/// that puts the opponent in **checkmate**.
///
/// `opponent_hand` enumerates the opponent's pieces in hand (for their possible
/// escaping moves, drops included; a list mixing both sides' pieces is
/// accepted — only the opponent's are considered). Returns `false` for any
/// token other than a Fu, and — per the single-square property (module doc) —
/// for any `to` whose forward square does not hold the opponent's royal.
///
/// The position is assumed coherent (the dropper is to move, the opponent not
/// already in check): on such positions the fast gate is exact.
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

    // Single-square gate: the dropped Fu's only attack is the square directly
    // in front of it; the drop can be mate only if the opponent's royal stands
    // there (see the module doc). A `to` on the last rank has no forward
    // square — such a drop is structurally illegal anyway.
    let royal_in_front = to
        .offset(0, forward(dropper))
        .and_then(&piece_at)
        .is_some_and(|piece| piece.belongs_to(opponent) && piece.is_royal());
    if !royal_in_front {
        return false;
    }

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

/// [`is_uchifuzume`] over a [`Position`]: collects the variant assignment, the
/// board closure, and the opponent's hand from the position itself.
///
/// This is the convenience the `engine` façade and the kernel share to guard a
/// resolved drop (`dropped` and `to` come from the resolved
/// [`crate::apply::Effect::Drop`]).
#[must_use]
pub fn is_uchifuzume_drop(position: &Position, dropped: Piece, to: Square) -> bool {
    // Cheap pre-filters before any allocation: only a Fu drop with the
    // opponent's royal in front can be an uchifuzume.
    if dropped.kind_letter() != 'F' {
        return false;
    }
    let opponent = dropped.side().flip();
    let opponent_hand: Vec<Piece> = position.hand(opponent).map(|(held, _)| held).collect();
    is_uchifuzume(
        dropped,
        to,
        position.variants(),
        |square| position.piece_at(square),
        &opponent_hand,
    )
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

    #[test]
    fn gate_requires_the_royal_directly_in_front() {
        // Single-square property: a Fu drop whose forward square does not hold
        // the opponent's royal is dismissed by the gate — here g7 (front = g8,
        // empty) in the very board where h7 IS an uchifuzume.
        let b = board(&[("h8", "k^"), ("g1", "R"), ("f6", "N")]);
        assert!(!is_uchifuzume(piece("F"), sq("g7"), OGI, &b, &[]));
        // A non-royal in front is dismissed the same way.
        let b2 = board(&[("h8", "r"), ("g1", "R"), ("f6", "N")]);
        assert!(!is_uchifuzume(piece("F"), sq("h7"), OGI, &b2, &[]));
        // A last-rank `to` has no forward square at all.
        assert!(!is_uchifuzume(piece("F"), sq("h8"), OGI, &b, &[]));
    }

    #[test]
    fn position_helper_matches_the_closure_form() {
        use super::is_uchifuzume_drop;
        use crate::position::Position;

        // The kernel's uchifuzume fixture: mating Fu drop on h7 (ōgi).
        let position =
            Position::parse("7k^/8/5N2/8/8/8/8/4K^1R1 F/ J/j").expect("valid Sanki FEEN");
        assert!(is_uchifuzume_drop(&position, piece("F"), sq("h7")));
        // A checking-but-not-mating square is allowed…
        let no_knight = Position::parse("7k^/8/8/8/8/8/8/4K^1R1 F/ J/j").expect("valid FEEN");
        assert!(!is_uchifuzume_drop(&no_knight, piece("F"), sq("h7")));
        // …and a non-Fu token is dismissed before any board work.
        assert!(!is_uchifuzume_drop(&position, piece("R"), sq("h7")));
    }
}
