//! Draw by **insufficient material**.
//!
//! Unified rule for the three variants (chess, ōgi, xiongqi), all matchups, and
//! **all royal pairings**: an immediate draw as soon as **only the two royals**
//! remain on the board **and** neither side holds **droppable** material in hand.
//!
//! - Chess / xiongqi: the hand is inert (captured pieces keep the opponent's case
//!   and can never come back); only the board state matters.
//! - Ōgi: any piece in hand bears the holder's case, hence droppable; the
//!   slightest piece in hand revives the game. King versus King with **empty
//!   hands** is the only insufficient configuration.
//!
//! The rule deliberately covers the **mixed King-versus-General pair** too
//! (interactions-*-xiongqi.md §End of Game). Strictly, that pair is not a dead
//! position — the General steps one square but attacks only along open lines, so
//! a diagonally adjacent King checks it unanswered, and a cornered General is
//! matable in principle (General a1 against King b2). But neither side can
//! **force** a win (best defence yields at most a stalemate), so Sashité declares
//! the position drawn outright rather than making the players shuffle to a
//! stalemate, repetition, or move-limit finish (deciders' ruling, 2026-07-10).
//!
//! The "droppable" criterion is universal: a piece in hand is droppable by its
//! holder iff it bears **its** case. An inert piece (opponent's case, coming from
//! a chess/xiongqi capture) therefore does not prevent the draw.

use crate::domain::piece::Piece;
use crate::domain::side::Side;
use crate::domain::square::Square;

/// True if the position is a draw by insufficient material: only royals remain on
/// the board and no hand holds a droppable piece.
///
/// `first_hand` / `second_hand` enumerate each player's pieces in hand (a piece is
/// droppable there iff it belongs to the holding side).
#[inline]
#[must_use]
pub fn is_insufficient_material(
    piece_at: impl Fn(Square) -> Option<Piece>,
    first_hand: &[Piece],
    second_hand: &[Piece],
) -> bool {
    board_holds_only_royals(&piece_at) && !any_droppable(first_hand, second_hand)
}

/// True if every piece present on the board is a royal.
fn board_holds_only_royals(piece_at: &impl Fn(Square) -> Option<Piece>) -> bool {
    Square::all().all(|square| match piece_at(square) {
        Some(piece) => piece.is_royal(),
        None => true,
    })
}

/// True if at least one hand holds a piece droppable by its holder.
fn any_droppable(first_hand: &[Piece], second_hand: &[Piece]) -> bool {
    first_hand.iter().any(|p| p.belongs_to(Side::First))
        || second_hand.iter().any(|p| p.belongs_to(Side::Second))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::is_insufficient_material;
    use crate::domain::piece::Piece;
    use crate::domain::square::Square;
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

    #[test]
    fn two_lone_kings_empty_hands() {
        // Chess / ōgi: King versus King.
        let b = board(&[("e1", "K^"), ("e8", "k^")]);
        assert!(is_insufficient_material(&b, &[], &[]));
        // Xiongqi: General versus General.
        let g = board(&[("e1", "G^"), ("e8", "g^")]);
        assert!(is_insufficient_material(&g, &[], &[]));
    }

    #[test]
    fn king_versus_general_draws_by_deliberate_ruling() {
        // Cross-variant matchups (chess/ōgi vs xiongqi): the mixed royal pair is
        // not strictly a dead position, but no side can force a win — declared
        // drawn outright (interactions-*-xiongqi.md §End of Game; deciders'
        // ruling, 2026-07-10). This test pins that decision.
        let b = board(&[("e1", "K^"), ("e8", "g^")]);
        assert!(is_insufficient_material(&b, &[], &[]));
        let g = board(&[("e1", "G^"), ("e8", "k^")]);
        assert!(is_insufficient_material(&g, &[], &[]));
    }

    #[test]
    fn board_piece_is_sufficient() {
        // A Rook remains: sufficient material.
        let b = board(&[("e1", "K^"), ("e8", "k^"), ("a1", "R")]);
        assert!(!is_insufficient_material(&b, &[], &[]));
    }

    #[test]
    fn droppable_hand_revives_game() {
        // Ōgi: a droppable Fu (first player's case) in their hand.
        let b = board(&[("e1", "K^"), ("e8", "k^")]);
        assert!(!is_insufficient_material(&b, &[piece("F")], &[]));
        // Same on the second player's side.
        assert!(!is_insufficient_material(&b, &[], &[piece("f")]));
    }

    #[test]
    fn inert_hand_does_not_prevent_draw() {
        // Chess: the first player captured a black pawn -> `p` inert in their hand
        // (opponent's case, not droppable). Does not prevent the draw.
        let b = board(&[("e1", "K^"), ("e8", "k^")]);
        assert!(is_insufficient_material(&b, &[piece("p")], &[piece("P")]));
    }
}
