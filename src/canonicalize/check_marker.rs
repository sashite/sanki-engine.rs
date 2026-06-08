//! Check marker (`-K^` / `-G^`) recomposition.
//!
//! A royal in check carries the diminished prefix (`-K^`/`-k^` for the King in
//! chess and ōgi, `-G^`/`-g^` for the General in xiongqi). The marker is purely
//! transient: per the variant rules it is *cleared and re-evaluated after each
//! move*. This recomposition therefore re-derives it from scratch on the
//! resulting position — it is a **pure function of the board**, needing neither
//! the move that produced the position nor any look-ahead.
//!
//! For each side, the royal's token is set to its diminished form when its
//! square is attacked by the opponent, and to its normal form otherwise. Under
//! legal play only the side to move can be in check, but evaluating both royals
//! costs nothing and keeps the function a faithful "marker iff in check" rule,
//! robust to any stale marker left on the input.

use crate::canonicalize::CanonicalizeError;
use crate::domain::side::Side;
use crate::domain::square::Square;
use crate::legality::check::{in_check, royal_square};
use crate::position::{board, Position};
use sashite_epin::Identifier as Epin;

/// Returns `position` with each royal's check marker re-derived: diminished
/// (`-`) when that royal is in check, normal otherwise.
///
/// # Errors
/// - [`CanonicalizeError::Qi`] if applying the token rewrites fails;
/// - [`CanonicalizeError::Position`] if the rewritten position is not Sanki.
///
/// Both are unreachable on a well-formed Sanki position (see the type's docs).
pub fn recompose(position: &Position) -> Result<Position, CanonicalizeError> {
    let piece_at = |square: Square| position.piece_at(square);
    let mut changes: Vec<(usize, Option<Epin>)> = Vec::with_capacity(2);

    for side in [Side::First, Side::Second] {
        // No royal for this side: nothing to mark (a malformed input the caller
        // handles elsewhere).
        let Some(royal_sq) = royal_square(side, piece_at) else {
            continue;
        };
        let Some(royal) = piece_at(royal_sq) else {
            continue;
        };

        let checked = in_check(side, position.variant_of(side.flip()), piece_at);

        // Rewrite only the state prefix, preserving letter, side, the terminal
        // `^` and the native/derived flag.
        let epin = royal.epin();
        let pin = epin.pin();
        let desired = epin.with_pin(if checked {
            pin.diminished()
        } else {
            pin.normalized()
        });

        if desired != epin {
            changes.push((board::to_index(royal_sq), Some(desired)));
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
    fn sets_marker_on_royal_in_check() {
        // White rook a8 checking the black King e8 along the rank; Black to move
        // (the side in check is the side to move).
        let p = pos("R3k^3/8/8/8/8/8/8/4K^3 / w/W");
        let after = recompose(&p).expect("recomposition");
        assert_eq!(
            token_at(&after, "e8"),
            Some("-k^".to_owned()),
            "black king in check"
        );
        // The white King is not attacked: no marker.
        assert_eq!(token_at(&after, "e1"), Some("K^".to_owned()));
    }

    #[test]
    fn removes_stale_marker() {
        // White King marked `-K^` but with no attacker: the marker is removed.
        let p = pos("4k^3/8/8/8/8/8/8/4-K^3 / W/w");
        let after = recompose(&p).expect("recomposition");
        assert_eq!(
            token_at(&after, "e1"),
            Some("K^".to_owned()),
            "stale marker removed"
        );
    }

    #[test]
    fn position_without_check_unchanged() {
        let feen = "4k^3/8/8/8/8/8/8/4K^3 / W/w";
        let after = recompose(&pos(feen)).expect("recomposition");
        assert_eq!(after.to_feen(), feen, "FEEN unchanged");
    }

    #[test]
    fn marks_the_xiongqi_general() {
        // White General e1 in check from a black Chariot e8 on the open e-file
        // (Chariot range); xiongqi styles (`C`), White to move.
        let p = pos("g^3r3/8/8/8/8/8/8/4G^3 / C/c");
        let after = recompose(&p).expect("recomposition");
        assert_eq!(
            token_at(&after, "e1"),
            Some("-G^".to_owned()),
            "white general in check"
        );
        // The black General is not threatened.
        assert_eq!(token_at(&after, "a8"), Some("g^".to_owned()));
    }
}
