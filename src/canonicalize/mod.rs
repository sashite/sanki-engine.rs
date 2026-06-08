//! Canonicalization: recomposes the transient board markers on the position
//! produced by a ply, so that the FEEN the kernel returns is canonical.
//!
//! [`canonicalize`] is the layer's entry point. It threads the position through
//! the four marker recompositions in a fixed order. Two are pure functions of
//! the board ([`check_marker`], [`double_step`]); the other two also need the
//! ply's [`Effect`] ([`en_passant_flag`], [`castling_rights`]). The
//! recompositions are mutually independent — none reads another's marker, since
//! attack and check tests are derived from board geometry rather than from the
//! transient prefixes — so the order is chosen for clarity, save for one
//! constraint: [`double_step`] runs before [`en_passant_flag`], stripping the
//! mutually-exclusive `+` from the foot soldier that just double-stepped before
//! the `-` is considered.

pub mod castling_rights;
pub mod check_marker;
pub mod double_step;
pub mod en_passant_flag;

use crate::apply::Effect;
use crate::position::{Position, PositionError};
use sashite_qi::Error as QiError;

/// Recomposes every transient marker on `position` — the board resulting from
/// applying `effect` — and returns the canonical next position.
///
/// `effect` is the ply just applied; it is consulted by the en passant and
/// castling recompositions, which depend on what moved.
///
/// # Errors
/// Forwards any [`CanonicalizeError`] raised by an underlying recomposition;
/// none occurs on a well-formed Sanki position.
pub fn canonicalize(position: &Position, effect: &Effect) -> Result<Position, CanonicalizeError> {
    let position = check_marker::recompose(position)?;
    let position = double_step::recompose(&position)?;
    let position = en_passant_flag::recompose(&position, effect)?;
    castling_rights::recompose(&position, effect)
}

/// Failure of a marker recomposition.
///
/// As in [`crate::apply`], these cases denote a broken internal invariant rather
/// than an illegal move: a recomposition only rewrites the `+`/`-` state of
/// tokens already on the board, so on a well-formed input none of them occurs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalizeError {
    /// A `Qi` board transformation failed (index, cardinality).
    Qi(QiError),
    /// The resulting position no longer has Sanki styles — unreachable, as
    /// styles are invariant across a game and markers leave them untouched.
    Position(PositionError),
}

impl core::fmt::Display for CanonicalizeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Qi(e) => write!(f, "position transformation refused: {e}"),
            Self::Position(e) => write!(f, "resulting position is not Sanki: {e}"),
        }
    }
}

impl core::error::Error for CanonicalizeError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Qi(e) => Some(e),
            Self::Position(e) => Some(e),
        }
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

    use super::canonicalize;
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
    fn capturable_double_step_becomes_minus_p() {
        // Post-apply: the moving pawn still carries `+P` on its destination square.
        // `double_step` strips the `+`, then `en_passant_flag` sets `-` because the
        // e.p. capture (d4→e3) is legal: the canonical result is `-P`.
        let p = pos("4k^3/8/8/8/3p+P3/8/8/4K^3 / w/W");
        let after = canonicalize(&p, &board_move("e2", "e4", "+P")).expect("canonicalization");
        assert_eq!(
            token_at(&after, "e4"),
            Some("-P".to_owned()),
            "double step capturable e.p."
        );
    }

    #[test]
    fn check_and_castling_right_recomposed_together() {
        // A white Rook arrives on a8 and puts the black King in check; the white
        // King keeps a performable castling. `check_marker` sets `-k^`, `castling`
        // confirms `+R` — both in the same pass.
        let p = pos("R3k^3/8/8/8/8/8/8/4K^2+R / w/W");
        let after = canonicalize(&p, &board_move("a1", "a8", "R")).expect("canonicalization");
        assert_eq!(
            token_at(&after, "e8"),
            Some("-k^".to_owned()),
            "black king in check"
        );
        assert_eq!(
            token_at(&after, "h1"),
            Some("+R".to_owned()),
            "white castling performable"
        );
    }

    #[test]
    fn canonical_position_unchanged() {
        // Nothing to recompose (no marker, no check): identical FEEN.
        let feen = "4k^3/8/8/8/8/8/8/4K^3 / W/w";
        let after =
            canonicalize(&pos(feen), &board_move("a2", "a3", "P")).expect("canonicalization");
        assert_eq!(after.to_feen(), feen, "FEEN unchanged");
    }
}
