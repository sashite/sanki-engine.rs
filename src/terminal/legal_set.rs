//! Legal-move enumeration — the **keystone** of legality.
//!
//! `has_legal_move` answers: "does the side to move have at least one legal
//! move?". It is the basis for checkmate (in check **and** no legal move) and
//! stalemate (not in check **and** no legal move).
//!
//! It **composes** the whole layer already built:
//! - pseudo-legal board moves ([`crate::movement::generate`]), filtered by
//!   self-check ([`crate::legality::self_check`]);
//! - en passant ([`crate::legality::en_passant`]);
//! - castling ([`crate::legality::castling`], chess only);
//! - ōgi drops ([`crate::legality::drops`]) — **without uchifuzume**, since a
//!   defensive drop that parries the check is legal; uchifuzume restricts only the
//!   player who *delivers* mate, and composes on top of this enumeration.
//!
//! To decide *legality*, the exact type of a promotion has no bearing on the
//! royal's safety (a promoted piece is never royal and blocks a line like any
//! piece); self-check is therefore tested with the moved piece as-is.
//!
//! Decoupled from `Position`: operates on a `piece_at` closure, the variant
//! assignment, and the list of pieces in hand.

use crate::domain::piece::Piece;
use crate::domain::side::Side;
use crate::domain::square::Square;
use crate::domain::variant::{Variant, VariantAssignment};
use crate::legality::castling::resolve_castling;
use crate::legality::drops::drop_is_legal;
use crate::legality::en_passant::en_passant_capture;
use crate::legality::self_check::move_is_safe;
use crate::movement::forward;
use crate::movement::generate::pseudo_legal_destinations;

/// True if side `side` has at least one legal move in the position described by
/// `piece_at`, under the variant assignment `variants` and with `hand` for pieces
/// in hand (the list may contain pieces of both sides; only `side`'s are
/// droppable).
#[inline]
#[must_use]
pub fn has_legal_move(
    side: Side,
    variants: VariantAssignment,
    piece_at: impl Fn(Square) -> Option<Piece>,
    hand: &[Piece],
) -> bool {
    has_move(side, variants, piece_at, hand, true)
}

/// True if side `side` has at least one **pseudo-legal** move — the set of moves
/// available *before* the self-check filter. This is the notion that distinguishes
/// `nomove` (empty pseudo-legal set) from stalemate and checkmate (non-empty
/// pseudo-legal set but empty legal set).
#[inline]
#[must_use]
pub fn has_pseudo_legal_move(
    side: Side,
    variants: VariantAssignment,
    piece_at: impl Fn(Square) -> Option<Piece>,
    hand: &[Piece],
) -> bool {
    has_move(side, variants, piece_at, hand, false)
}

/// Common core: existence of at least one half-move for `side`, the self-check
/// filter being applied iff `require_safe`.
///
/// Castling is judged by `resolve_castling` (full legality, including the FIDE
/// check conditions) in both modes: it is a subset of the pseudo-legal set, so a
/// castling blocked by a check is not counted on the pseudo side — an acceptable
/// corner, the King's own pseudo-legal moves being counted elsewhere.
fn has_move(
    side: Side,
    variants: VariantAssignment,
    piece_at: impl Fn(Square) -> Option<Piece>,
    hand: &[Piece],
    require_safe: bool,
) -> bool {
    let own_variant = variants.variant_of(side);
    let opponent_variant = variants.variant_of(side.flip());
    let occupant_side = |square: Square| piece_at(square).map(Piece::side);

    // 1. Board moves (and en passant) of each piece of the side.
    for from in Square::all() {
        let Some(piece) = piece_at(from) else {
            continue;
        };
        if !piece.belongs_to(side) {
            continue;
        }

        // Pseudo-legal destinations filtered by self-check.
        for to in pseudo_legal_destinations(
            own_variant,
            side,
            piece.kind_letter(),
            piece.is_enhanced(),
            from,
            occupant_side,
        ) {
            if !require_safe
                || move_is_safe(
                    side,
                    opponent_variant,
                    Some(from),
                    to,
                    piece,
                    None,
                    &piece_at,
                )
            {
                return true;
            }
        }

        // En passant: the foot soldier's capture pattern toward an empty skipped
        // square. We try its candidate capture squares; `en_passant_capture`
        // validates the variant-specific geometry.
        if piece.is_foot_soldier() {
            let fwd = forward(side);
            for (df, dr) in [(-1, fwd), (1, fwd), (-1, 0), (1, 0)] {
                let Some(to) = from.offset(df, dr) else {
                    continue;
                };
                if let Some(captured) = en_passant_capture(own_variant, side, from, to, &piece_at) {
                    if !require_safe
                        || move_is_safe(
                            side,
                            opponent_variant,
                            Some(from),
                            to,
                            piece,
                            Some(captured),
                            &piece_at,
                        )
                    {
                        return true;
                    }
                }
            }
        }
    }

    // 2. Castling (chess only; `resolve_castling` checks all the legality).
    if own_variant == Variant::Chess {
        let rank = home_rank(side);
        if let Some(king_from) = Square::new(KING_FILE, rank) {
            for to_file in [KINGSIDE_FILE, QUEENSIDE_FILE] {
                if let Some(king_to) = Square::new(to_file, rank) {
                    if resolve_castling(side, opponent_variant, king_from, king_to, &piece_at)
                        .is_some()
                    {
                        return true;
                    }
                }
            }
        }
    }

    // 3. Drops (ōgi). We try only the side's droppable pieces; a drop that parries
    // the check is legal (uchifuzume is applied separately).
    for &dropped in hand {
        if !dropped.belongs_to(side) {
            continue;
        }
        for to in Square::all() {
            if piece_at(to).is_some() {
                continue;
            }
            if drop_is_legal(dropped, to, &piece_at).is_ok()
                && (!require_safe
                    || move_is_safe(side, opponent_variant, None, to, dropped, None, &piece_at))
            {
                return true;
            }
        }
    }

    false
}

/// King's home file (file `e`).
const KING_FILE: u8 = 4;
/// Kingside castling destination file (file `g`).
const KINGSIDE_FILE: u8 = 6;
/// Queenside castling destination file (file `c`).
const QUEENSIDE_FILE: u8 = 2;

/// King's home rank for side `side`.
#[inline]
const fn home_rank(side: Side) -> u8 {
    match side {
        Side::First => 0,
        Side::Second => Square::RANK_COUNT - 1,
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

    use super::{has_legal_move, has_pseudo_legal_move};
    use crate::domain::piece::Piece;
    use crate::domain::side::Side;
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

    const CHESS: VariantAssignment = VariantAssignment {
        first: Variant::Chess,
        second: Variant::Chess,
    };
    const OGI: VariantAssignment = VariantAssignment {
        first: Variant::Ogi,
        second: Variant::Ogi,
    };

    #[test]
    fn checkmate_no_move() {
        // Back-rank mate: King g1 walled in by its pawns, black Rook on e1.
        let b = board(&[
            ("g1", "K^"),
            ("f2", "P"),
            ("g2", "P"),
            ("h2", "P"),
            ("e1", "r"),
        ]);
        assert!(!has_legal_move(Side::First, CHESS, &b, &[]));
    }

    #[test]
    fn mate_has_a_non_empty_pseudo_legal_set() {
        // Same back-rank mate: no legal move, but the King has pseudo-legal squares
        // (f1, h1, attacked) → it is not a `nomove`.
        let b = board(&[
            ("g1", "K^"),
            ("f2", "P"),
            ("g2", "P"),
            ("h2", "P"),
            ("e1", "r"),
        ]);
        assert!(!has_legal_move(Side::First, CHESS, &b, &[]));
        assert!(has_pseudo_legal_move(Side::First, CHESS, &b, &[]));
    }

    #[test]
    fn normal_position_has_pseudo_legal_moves() {
        let b = board(&[("e1", "K^"), ("e8", "k^"), ("a1", "R")]);
        assert!(has_pseudo_legal_move(Side::First, CHESS, &b, &[]));
    }

    #[test]
    fn no_piece_gives_an_empty_pseudo_legal_set() {
        // Degenerate board with no first-side piece and no reserve: empty
        // pseudo-legal set (the `nomove` branch).
        let b = board(&[("e8", "k^")]);
        assert!(!has_pseudo_legal_move(Side::First, CHESS, &b, &[]));
    }

    #[test]
    fn stalemate_no_move_without_check() {
        // Classic stalemate: black King h8, white King f7, white Queen g6.
        let b = board(&[("h8", "k^"), ("f7", "K^"), ("g6", "Q")]);
        assert!(!has_legal_move(Side::Second, CHESS, &b, &[]));
    }

    #[test]
    fn position_with_legal_moves() {
        // King and Rook free: many moves exist.
        let b = board(&[("e1", "K^"), ("a1", "R"), ("e8", "k^")]);
        assert!(has_legal_move(Side::First, CHESS, &b, &[]));
    }

    #[test]
    fn capturing_the_checker_escapes() {
        // King in check from an adjacent undefended Rook: the King captures it.
        let b = board(&[("e1", "K^"), ("e2", "r"), ("a8", "k^")]);
        assert!(has_legal_move(Side::First, CHESS, &b, &[]));
    }

    #[test]
    fn castling_as_a_legal_move() {
        // An available kingside castling counts as a legal move.
        let b = board(&[("e1", "K^"), ("h1", "+R")]);
        assert!(has_legal_move(Side::First, CHESS, &b, &[]));
    }

    #[test]
    fn ogi_drop_parries_check() {
        // King a1 walled in by two enemy Rooks (files a and b): no board move, but
        // a Fu in hand parries the check on the a-file.
        let cases = &[("a1", "K^"), ("a8", "r"), ("b8", "r")];
        let b = board(cases);
        // Without a hand: checkmate.
        assert!(!has_legal_move(Side::First, OGI, &b, &[]));
        // With a Fu in hand: the blocking drop yields a legal move.
        assert!(has_legal_move(Side::First, OGI, &b, &[piece("F")]));
    }

    #[test]
    fn drop_that_does_not_parry_is_not_enough() {
        // Same position: a Fu already present on the a-file (nifu) cannot be
        // re-dropped, and anyway a drop off the a-file does not parry. Here we check
        // that an enemy piece in hand is not droppable.
        let b = board(&[("a1", "K^"), ("a8", "r"), ("b8", "r")]);
        // A piece of the opponent's case (lowercase) does not belong to First.
        assert!(!has_legal_move(Side::First, OGI, &b, &[piece("f")]));
    }
}
