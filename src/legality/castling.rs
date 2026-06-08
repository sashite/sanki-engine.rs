//! Castling — resolution and legality (specific to **chess**; ōgi and xiongqi
//! have no castling).
//!
//! Castling is a King + Rook move in a single turn, following the FIDE rules. It
//! is detected when a King moves **two squares horizontally** from its origin
//! square (`e1`/`e8`). The six FIDE conditions:
//!
//! 1. the King has never moved;
//! 2. the relevant Rook carries the castling right (`+R`);
//! 3. the squares **between** the King and the Rook are empty;
//! 4. the King is **not in check**;
//! 5. the King does **not pass through** an attacked square;
//! 6. the King does **not land** on an attacked square.
//!
//! The `+R` marker alone encodes conditions 1‑2: canonicalization removes `+R`
//! from both Rooks as soon as the King moves, and from the relevant Rook as soon
//! as **it** moves. We nonetheless check 3‑6 on the current board, which makes
//! this module independent of the markers' correctness (defense in depth) and
//! reusable by canonicalization.
//!
//! Decoupled from `Position`: operates on a `piece_at` closure and the opponent's
//! variant (needed for attacks in cross-variant play).

use crate::domain::piece::Piece;
use crate::domain::side::Side;
use crate::domain::square::Square;
use crate::domain::variant::Variant;
use crate::movement::attack::is_attacked;

/// King's origin file (file `e`).
const KING_FILE: u8 = 4;
/// Index of the last rank (the first player's rank 8).
const LAST_RANK: u8 = Square::RANK_COUNT - 1;

// Per-side geometry, expressed as files on the King's home rank.
// `between`: squares that must be empty; `path`: squares occupied or crossed by
// the King, which must not be attacked (the King's file included, for the
// "not in check" condition).
const KINGSIDE_BETWEEN: [u8; 2] = [5, 6]; // f, g
const KINGSIDE_PATH: [u8; 3] = [4, 5, 6]; // e, f, g
const QUEENSIDE_BETWEEN: [u8; 3] = [1, 2, 3]; // b, c, d
const QUEENSIDE_PATH: [u8; 3] = [4, 3, 2]; // e, d, c

/// A castling's board effect: the King's and Rook's moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastlingMove {
    /// King's origin square.
    pub king_from: Square,
    /// King's destination square.
    pub king_to: Square,
    /// Rook's origin square.
    pub rook_from: Square,
    /// Rook's destination square.
    pub rook_to: Square,
}

/// If `king_from -> king_to` is a **legal** castling for side `side`, returns its
/// effect (King's and Rook's moves); otherwise `None`.
///
/// `opponent_variant` is the opponent's variant, whose pieces may attack the
/// King's path.
#[must_use]
pub fn resolve_castling(
    side: Side,
    opponent_variant: Variant,
    king_from: Square,
    king_to: Square,
    piece_at: impl Fn(Square) -> Option<Piece>,
) -> Option<CastlingMove> {
    let rank = home_rank(side);

    // Conditions 1‑2 (King part): a King of the right side on its origin square.
    if king_from != Square::new(KING_FILE, rank)? {
        return None;
    }
    let king = piece_at(king_from)?;
    if king.kind_letter() != 'K' || !king.belongs_to(side) {
        return None;
    }

    // Castling side, determined by the King's destination square.
    let kingside = Square::new(6, rank)?;
    let queenside = Square::new(2, rank)?;
    let (rook_from_file, rook_to_file, between, path): (u8, u8, &[u8], &[u8]) =
        if king_to == kingside {
            (7, 5, &KINGSIDE_BETWEEN, &KINGSIDE_PATH)
        } else if king_to == queenside {
            (0, 3, &QUEENSIDE_BETWEEN, &QUEENSIDE_PATH)
        } else {
            return None; // not a castling (the King does not go to c/g)
        };
    let rook_from = Square::new(rook_from_file, rank)?;
    let rook_to = Square::new(rook_to_file, rank)?;

    // Conditions 1‑2 (Rook part): a Rook of the right side carrying `+R`.
    let rook = piece_at(rook_from)?;
    if rook.kind_letter() != 'R' || !rook.belongs_to(side) || !rook.is_enhanced() {
        return None;
    }

    // Condition 3: intermediate squares empty.
    for &file in between {
        if piece_at(Square::new(file, rank)?).is_some() {
            return None;
        }
    }

    // Conditions 4‑6: no square on the King's path is attacked.
    let opponent = side.flip();
    for &file in path {
        if is_attacked(
            Square::new(file, rank)?,
            opponent,
            opponent_variant,
            &piece_at,
        ) {
            return None;
        }
    }

    Some(CastlingMove {
        king_from,
        king_to,
        rook_from,
        rook_to,
    })
}

/// King's home rank for side `side`.
#[inline]
const fn home_rank(side: Side) -> u8 {
    match side {
        Side::First => 0,
        Side::Second => LAST_RANK,
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

    use super::{resolve_castling, CastlingMove};
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

    fn castle(king_to: &str, b: impl Fn(Square) -> Option<Piece>) -> Option<CastlingMove> {
        resolve_castling(Side::First, Variant::Chess, sq("e1"), sq(king_to), b)
    }

    #[test]
    fn kingside_castling_legal() {
        let b = board(&[("e1", "K^"), ("h1", "+R")]);
        assert_eq!(
            castle("g1", &b),
            Some(CastlingMove {
                king_from: sq("e1"),
                king_to: sq("g1"),
                rook_from: sq("h1"),
                rook_to: sq("f1"),
            })
        );
    }

    #[test]
    fn queenside_castling_legal() {
        let b = board(&[("e1", "K^"), ("a1", "+R")]);
        assert_eq!(
            castle("c1", &b),
            Some(CastlingMove {
                king_from: sq("e1"),
                king_to: sq("c1"),
                rook_from: sq("a1"),
                rook_to: sq("d1"),
            })
        );
    }

    #[test]
    fn rook_without_right_refused() {
        // Rook without `+R` (has already moved).
        let b = board(&[("e1", "K^"), ("h1", "R")]);
        assert_eq!(castle("g1", &b), None);
    }

    #[test]
    fn intermediate_square_occupied_refused() {
        // Knight on g1: kingside castling impossible.
        let b = board(&[("e1", "K^"), ("h1", "+R"), ("g1", "N")]);
        assert_eq!(castle("g1", &b), None);
        // For queenside castling, b1 must also be empty (without being crossed).
        let b2 = board(&[("e1", "K^"), ("a1", "+R"), ("b1", "n")]);
        assert_eq!(castle("c1", &b2), None);
    }

    #[test]
    fn king_in_check_refused() {
        // Enemy rook on the e-file: King in check (condition 4).
        let b = board(&[("e1", "K^"), ("h1", "+R"), ("e8", "r")]);
        assert_eq!(castle("g1", &b), None);
    }

    #[test]
    fn king_crosses_attacked_square_refused() {
        // Enemy rook attacking f1: the King would cross an attacked square (5).
        let b = board(&[("e1", "K^"), ("h1", "+R"), ("f8", "r")]);
        assert_eq!(castle("g1", &b), None);
    }

    #[test]
    fn king_lands_on_attacked_square_refused() {
        // Enemy rook attacking g1: destination square attacked (condition 6).
        let b = board(&[("e1", "K^"), ("h1", "+R"), ("g8", "r")]);
        assert_eq!(castle("g1", &b), None);
    }

    #[test]
    fn attacked_square_off_path_allowed() {
        // Queenside castling crosses e,d,c; b1 attacked does not prevent it (off path).
        let b = board(&[("e1", "K^"), ("a1", "+R"), ("b8", "r")]);
        assert!(castle("c1", &b).is_some());
    }

    #[test]
    fn not_a_castling_refused() {
        // King by a single square: this is not a castling.
        let b = board(&[("e1", "K^"), ("h1", "+R")]);
        assert_eq!(castle("f1", &b), None);
    }

    #[test]
    fn second_player_kingside_castling() {
        let b = board(&[("e8", "k^"), ("h8", "+r")]);
        assert_eq!(
            resolve_castling(Side::Second, Variant::Chess, sq("e8"), sq("g8"), &b),
            Some(CastlingMove {
                king_from: sq("e8"),
                king_to: sq("g8"),
                rook_from: sq("h8"),
                rook_to: sq("f8"),
            })
        );
    }

    #[test]
    fn cross_variant_flying_general_blocks() {
        // Xiongqi opponent: its General on e8 attacks e1 at Chariot range, so the
        // King is in check and cannot castle.
        let b = board(&[("e1", "K^"), ("h1", "+R"), ("e8", "g^")]);
        assert_eq!(
            resolve_castling(Side::First, Variant::Xiongqi, sq("e1"), sq("g1"), &b),
            None
        );
    }
}
