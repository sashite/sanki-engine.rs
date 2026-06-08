//! Castling-rights marker (`+R` / `-R` / `R`) recomposition (chess only).
//!
//! A chess Rook encodes its castling state in its prefix:
//! - `R` — the right is **lost** (the Rook, or its King, has moved); permanent;
//! - `+R` — the right is **retained and the castling is immediately performable**
//!   right now (FIDE conditions 3–6: squares between empty, King not in check,
//!   King neither passes through nor lands on an attacked square);
//! - `-R` — the right is **retained but transiently blocked** (some condition
//!   currently prevents castling).
//!
//! The marker carries the history that cannot be re-derived from the board
//! alone (whether the Rook/King have ever moved). This recomposition therefore
//! **preserves** the retained/lost distinction and only recomputes the `+`/`-`
//! sub-state each ply — except it **strips the right to `R`** when this ply moved
//! the King (both Rooks of that side) or moved a Rook onto/away from its origin
//! (that Rook). It never resurrects a lost right: a plain `R` is left untouched.
//!
//! Castling is chess-only, and only chess Rooks ever bear `+R`/`-R` (ōgi's sole
//! promoted form is the Tokin `T`; xiongqi never prefixes a promotion). The
//! recomputation reuses [`crate::legality::castling::resolve_castling`], which
//! gates on the `+R` marker, so performability is probed against a tentative
//! board where the Rook is provisionally `+R`.

use crate::apply::Effect;
use crate::canonicalize::CanonicalizeError;
use crate::domain::piece::Piece;
use crate::domain::side::Side;
use crate::domain::square::Square;
use crate::domain::variant::Variant;
use crate::legality::castling::resolve_castling;
use crate::position::{board, Position};
use sashite_epin::Identifier as Epin;

/// File of the King's home square (column `e`).
const KING_FILE: u8 = 4;
/// File the King reaches when castling kingside (column `g`).
const KINGSIDE_KING_FILE: u8 = 6;
/// File the King reaches when castling queenside (column `c`).
const QUEENSIDE_KING_FILE: u8 = 2;
/// File of the queenside Rook's origin (column `a`).
const QUEENSIDE_ROOK_FILE: u8 = 0;
/// File of the kingside Rook's origin (column `h`).
const KINGSIDE_ROOK_FILE: u8 = 7;
/// Home rank of the first player.
const HOME_RANK_FIRST: u8 = 0;
/// Home rank of the second player.
const HOME_RANK_SECOND: u8 = Square::RANK_COUNT - 1;

/// Returns `position` with chess castling markers recomputed: `+R`/`-R` for a
/// Rook that still holds the right (performable now, or transiently blocked),
/// and `R` once the right is lost this ply.
///
/// # Errors
/// - [`CanonicalizeError::Qi`] if applying the token rewrites fails;
/// - [`CanonicalizeError::Position`] if the rewritten position is not Sanki.
///
/// Both are unreachable on a well-formed Sanki position.
pub fn recompose(position: &Position, effect: &Effect) -> Result<Position, CanonicalizeError> {
    let king_moved = king_moved_side(effect);
    let mut changes: Vec<(usize, Option<Epin>)> = Vec::new();

    for square in Square::all() {
        let Some(rook) = position.piece_at(square) else {
            continue;
        };
        let side = rook.side();

        // Castling markers live only on chess Rooks.
        if rook.kind_letter() != 'R' || position.variant_of(side) != Variant::Chess {
            continue;
        }
        // A plain `R` has already lost the right; it is never resurrected.
        if !(rook.is_enhanced() || rook.is_diminished()) {
            continue;
        }

        // The right is retained by the marker; settle its fate this ply.
        let lost = !is_castling_origin(side, square)
            || arrived_this_ply(effect, square)
            || king_moved == Some(side);

        let epin = rook.epin();
        let pin = epin.pin();
        let desired = if lost {
            epin.with_pin(pin.normalized())
        } else if castling_performable(position, side, square) {
            epin.with_pin(pin.enhanced())
        } else {
            epin.with_pin(pin.diminished())
        };

        if desired != epin {
            changes.push((board::to_index(square), Some(desired)));
        }
    }

    if changes.is_empty() {
        return Ok(position.clone());
    }

    let qi = position
        .qi()
        .clone()
        .board_diff(changes)
        .map_err(CanonicalizeError::Qi)?;
    Position::new(qi).map_err(CanonicalizeError::Position)
}

/// The side whose King moved on the ply described by `effect`, if any. A King
/// move forfeits castling for **both** of that side's Rooks.
fn king_moved_side(effect: &Effect) -> Option<Side> {
    match effect {
        Effect::Board { placed, .. } if placed.kind_letter() == 'K' => Some(placed.side()),
        Effect::Castle(castling) => side_of_home_rank(castling.king_from.rank()),
        _ => None,
    }
}

/// Whether the ply described by `effect` moved a piece **onto** `square`. A Rook
/// landing on an origin square this ply has moved, so it cannot retain a right.
fn arrived_this_ply(effect: &Effect, square: Square) -> bool {
    match effect {
        Effect::Board { to, .. } | Effect::Drop { to, .. } => *to == square,
        Effect::Castle(castling) => castling.king_to == square || castling.rook_to == square,
    }
}

/// Whether the King of `side` can castle **right now** with the Rook on
/// `rook_square`, probed against a board where that Rook provisionally bears the
/// `+R` marker that [`resolve_castling`] requires.
fn castling_performable(position: &Position, side: Side, rook_square: Square) -> bool {
    let rank = home_rank(side);
    let king_to_file = if rook_square.file() == KINGSIDE_ROOK_FILE {
        KINGSIDE_KING_FILE
    } else {
        QUEENSIDE_KING_FILE
    };
    let (Some(king_from), Some(king_to)) = (
        Square::new(KING_FILE, rank),
        Square::new(king_to_file, rank),
    ) else {
        return false;
    };

    let rook = match position.piece_at(rook_square) {
        Some(piece) => piece.epin(),
        None => return false,
    };
    let rook_enhanced = rook.with_pin(rook.pin().enhanced());
    let tentative = |square: Square| {
        if square == rook_square {
            Some(Piece::new(rook_enhanced))
        } else {
            position.piece_at(square)
        }
    };

    resolve_castling(
        side,
        position.variant_of(side.flip()),
        king_from,
        king_to,
        tentative,
    )
    .is_some()
}

/// Whether `square` is a castling-origin square (an `a`- or `h`-file Rook square
/// on `side`'s home rank).
#[inline]
const fn is_castling_origin(side: Side, square: Square) -> bool {
    square.rank() == home_rank(side)
        && (square.file() == QUEENSIDE_ROOK_FILE || square.file() == KINGSIDE_ROOK_FILE)
}

/// Home rank of `side`.
#[inline]
const fn home_rank(side: Side) -> u8 {
    match side {
        Side::First => HOME_RANK_FIRST,
        Side::Second => HOME_RANK_SECOND,
    }
}

/// The side whose home rank is `rank`, if it is a home rank.
#[inline]
const fn side_of_home_rank(rank: u8) -> Option<Side> {
    if rank == HOME_RANK_FIRST {
        Some(Side::First)
    } else if rank == HOME_RANK_SECOND {
        Some(Side::Second)
    } else {
        None
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

    use super::recompose;
    use crate::apply::Effect;
    use crate::domain::piece::Piece;
    use crate::domain::square::Square;
    use crate::position::Position;
    use sashite_epin::Identifier as Epin;

    fn pos(feen: &str) -> Position {
        Position::parse(feen).expect("valid Sanki FEEN")
    }

    fn sq(s: &str) -> Square {
        Square::parse(s).expect("valid square")
    }

    fn piece(token: &str) -> Piece {
        Piece::new(Epin::parse(token).expect("valid EPIN token"))
    }

    fn token_at(position: &Position, square: &str) -> Option<String> {
        position.piece_at(sq(square)).map(|p| p.to_string())
    }

    fn board_move(from: &str, to: &str, placed: &str) -> Effect {
        Effect::Board {
            from: sq(from),
            to: sq(to),
            placed: piece(placed),
            captured: None,
        }
    }

    #[test]
    fn promotes_to_plus_r_when_castling_becomes_performable_again() {
        // Rook h1 blocked (`-R`); the Bishop vacates g1 (now f3) this ply, freeing
        // f1/g1. Castling becomes performable again → `+R`.
        let p = pos("4k^3/8/8/8/8/5B2/8/4K^2-R / w/W");
        let after = recompose(&p, &board_move("g1", "f3", "B")).expect("recomposition");
        assert_eq!(
            token_at(&after, "h1"),
            Some("+R".to_owned()),
            "castling performable"
        );
    }

    #[test]
    fn downgrades_to_minus_r_when_king_in_check() {
        // White King e1 in check from the black Rook e8: castling blocked → `-R`.
        let p = pos("k^3r3/8/8/8/8/8/8/4K^2+R / W/w");
        let after = recompose(&p, &board_move("h8", "e8", "r")).expect("recomposition");
        assert_eq!(
            token_at(&after, "h1"),
            Some("-R".to_owned()),
            "castling transiently blocked"
        );
    }

    #[test]
    fn king_move_removes_right_from_both_rooks() {
        // The white King just played e1→e2: both Rooks lose the right.
        let p = pos("4k^3/8/8/8/8/8/4K^3/+R6+R / w/W");
        let after = recompose(&p, &board_move("e1", "e2", "K^")).expect("recomposition");
        assert_eq!(token_at(&after, "a1"), Some("R".to_owned()));
        assert_eq!(token_at(&after, "h1"), Some("R".to_owned()));
    }

    #[test]
    fn rook_arriving_on_an_origin_loses_right() {
        // The `+R` Rook slid from h1 to a1 this ply: stale marker removed.
        let p = pos("4k^3/8/8/8/8/8/8/+R3K^3 / w/W");
        let after = recompose(&p, &board_move("h1", "a1", "+R")).expect("recomposition");
        assert_eq!(
            token_at(&after, "a1"),
            Some("R".to_owned()),
            "right lost (Rook moved)"
        );
    }

    #[test]
    fn does_not_resurrect_a_lost_right() {
        // `R` Rook (right already lost) with a clear path: stays `R`, never
        // promoted to `+R`.
        let p = pos("3k^4/8/8/8/8/8/8/4K^2R / W/w");
        let after = recompose(&p, &board_move("e8", "d8", "k^")).expect("recomposition");
        assert_eq!(token_at(&after, "h1"), Some("R".to_owned()));
    }
}
