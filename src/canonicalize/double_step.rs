//! Double-step marker (`+P` / `+F` / `+S`) recomposition.
//!
//! Every foot soldier (Pawn `P`, Fu `F`, Soldier `S`) starts on its home rank
//! carrying the enhanced prefix `+`, marking eligibility for the double step.
//! Per the variant rules the prefix is **stripped on the foot soldier's first
//! move** (single or double step) and **never returns** — and a *dropped* foot
//! soldier enters without it, even onto the home rank.
//!
//! This recomposition is therefore **strip-only and purely positional**: it
//! never adds `+` (eligibility is established by the initial setup, not here),
//! and it removes `+` from any foot soldier that is **not on its own home
//! rank**. That single rule is exhaustive:
//! - foot soldiers move only forward and never return, so a `+`-marked foot
//!   soldier on its home rank has not moved and keeps the marker;
//! - a `+`-marked foot soldier anywhere else has moved (or `apply` carried the
//!   mover's `+` token onto the destination) and is stripped;
//! - a markerless foot soldier on the home rank was dropped there, so leaving it
//!   markerless is correct — adding `+` would be wrong.
//!
//! Ordering note: this runs before [`crate::canonicalize::en_passant_flag`],
//! which may then set the mutually exclusive `-` marker on a foot soldier that
//! just double-stepped.

use crate::canonicalize::CanonicalizeError;
use crate::domain::side::Side;
use crate::domain::square::Square;
use crate::position::{board, Position};
use sashite_epin::Identifier as Epin;

/// Home rank of the first player's foot soldiers (rank 2).
const HOME_RANK_FIRST: u8 = 1;
/// Home rank of the second player's foot soldiers (rank 7).
const HOME_RANK_SECOND: u8 = Square::RANK_COUNT - 2;

/// Returns `position` with stale double-step markers stripped: any foot soldier
/// carrying `+` while off its home rank is normalized.
///
/// # Errors
/// - [`CanonicalizeError::Qi`] if applying the token rewrites fails;
/// - [`CanonicalizeError::Position`] if the rewritten position is not Sanki.
///
/// Both are unreachable on a well-formed Sanki position.
pub fn recompose(position: &Position) -> Result<Position, CanonicalizeError> {
    let mut changes: Vec<(usize, Option<Epin>)> = Vec::new();

    for square in Square::all() {
        let Some(piece) = position.piece_at(square) else {
            continue;
        };
        // Only a foot soldier still flagged eligible while off its home rank
        // needs correcting.
        if piece.is_foot_soldier()
            && piece.is_enhanced()
            && square.rank() != home_rank(piece.side())
        {
            let epin = piece.epin();
            changes.push((
                board::to_index(square),
                Some(epin.with_pin(epin.pin().normalized())),
            ));
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

/// Home (starting) rank of `side`'s foot soldiers.
#[inline]
const fn home_rank(side: Side) -> u8 {
    match side {
        Side::First => HOME_RANK_FIRST,
        Side::Second => HOME_RANK_SECOND,
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
    use crate::domain::square::Square;
    use crate::position::Position;

    fn pos(feen: &str) -> Position {
        Position::parse(feen).expect("valid Sanki FEEN")
    }

    fn sq(s: &str) -> Square {
        Square::parse(s).expect("valid square")
    }

    fn token_at(position: &Position, square: &str) -> Option<String> {
        position.piece_at(sq(square)).map(|p| p.to_string())
    }

    #[test]
    fn strips_white_pawn_off_home_rank() {
        // `+P` on e4 (has already advanced): the marker is removed.
        let after = recompose(&pos("4k^3/8/8/8/4+P3/8/8/4K^3 / W/w")).expect("recomposition");
        assert_eq!(token_at(&after, "e4"), Some("P".to_owned()));
    }

    #[test]
    fn keeps_white_pawn_on_home_rank() {
        // `+P` on e2 (rank 2): marker kept, FEEN unchanged.
        let feen = "4k^3/8/8/8/8/8/4+P3/4K^3 / W/w";
        let after = recompose(&pos(feen)).expect("recomposition");
        assert_eq!(token_at(&after, "e2"), Some("+P".to_owned()));
        assert_eq!(after.to_feen(), feen, "FEEN unchanged");
    }

    #[test]
    fn strips_black_pawn_off_home_rank() {
        // `+p` (second player) on e5: off its home rank (rank 7).
        let after = recompose(&pos("4k^3/8/8/4+p3/8/8/8/4K^3 / w/W")).expect("recomposition");
        assert_eq!(token_at(&after, "e5"), Some("p".to_owned()));
    }

    #[test]
    fn dropped_fu_does_not_gain_marker() {
        // A markerless Fu on the home rank (dropped) never receives `+`:
        // recomposition only strips.
        let feen = "4k^3/8/8/8/8/8/4F3/4K^3 / J/j";
        let after = recompose(&pos(feen)).expect("recomposition");
        assert_eq!(token_at(&after, "e2"), Some("F".to_owned()));
        assert_eq!(after.to_feen(), feen, "FEEN unchanged");
    }

    #[test]
    fn strips_xiongqi_soldier_off_home_rank() {
        // `+S` (xiongqi) on e5: stale marker removed (covers the letter S).
        let after = recompose(&pos("4g^3/8/8/4+S3/8/8/8/4G^3 / C/c")).expect("recomposition");
        assert_eq!(token_at(&after, "e5"), Some("S".to_owned()));
    }
}
