//! En passant marker (`-P` / `-F` / `-S`) recomposition.
//!
//! A foot soldier that has just double-stepped carries the diminished prefix `-`
//! **only if** an en passant capture of it is actually legal on the immediately
//! following ply — the canonicality condition shared by the variants:
//!
//! 1. at least one opponent foot soldier is positioned to make the capture, and
//! 2. performing that capture would not leave the capturing player's royal in
//!    check.
//!
//! Otherwise the foot soldier stays unmarked. The marker lasts a single turn, so
//! this recomposition also **clears any stale `-`** left by an earlier ply: only
//! the foot soldier that double-stepped on the ply just played may end up
//! marked. It therefore depends on that ply's [`Effect`], unlike the purely
//! positional [`crate::canonicalize::double_step`] (which runs first and strips
//! the mutually exclusive `+`).
//!
//! Note the seam with [`crate::legality::en_passant`]: `en_passant_capture`
//! gates the *capture* on the victim already carrying `-`. To decide whether to
//! *set* `-`, the look-ahead here probes that same geometry against a tentative
//! board in which the victim is provisionally diminished, then checks the
//! capturer's self-safety against the real board.

use crate::apply::Effect;
use crate::canonicalize::CanonicalizeError;
use crate::domain::piece::Piece;
use crate::domain::square::Square;
use crate::legality::en_passant::en_passant_capture;
use crate::legality::self_check::move_is_safe;
use crate::position::{board, Position};
use sashite_epin::Identifier as Epin;

/// Returns `position` with the en passant marker recomposed: a fresh `-` set on
/// the foot soldier that just double-stepped when an en passant capture of it is
/// legal next ply, and every other (stale) `-` cleared.
///
/// # Errors
/// - [`CanonicalizeError::Qi`] if applying the token rewrites fails;
/// - [`CanonicalizeError::Position`] if the rewritten position is not Sanki.
///
/// Both are unreachable on a well-formed Sanki position.
pub fn recompose(position: &Position, effect: &Effect) -> Result<Position, CanonicalizeError> {
    // Square (if any) that this ply leaves capturable en passant.
    let flagged = match double_step_info(effect) {
        Some((from, to, placed)) => ep_flag_square(position, from, to, placed),
        None => None,
    };

    let mut changes: Vec<(usize, Option<Epin>)> = Vec::new();
    for square in Square::all() {
        let Some(piece) = position.piece_at(square) else {
            continue;
        };
        if !piece.is_foot_soldier() {
            continue;
        }
        let epin = piece.epin();
        if Some(square) == flagged {
            // Freshly capturable: ensure the diminished marker is present.
            if !piece.is_diminished() {
                changes.push((
                    board::to_index(square),
                    Some(epin.with_pin(epin.pin().diminished())),
                ));
            }
        } else if piece.is_diminished() {
            // Stale marker from an earlier ply: the window has closed.
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

/// If `effect` is a straight two-rank foot-soldier advance (a double step),
/// returns its source, destination and placed piece; otherwise `None`. A double
/// step never captures and never changes file.
fn double_step_info(effect: &Effect) -> Option<(Square, Square, Piece)> {
    let Effect::Board {
        from,
        to,
        placed,
        captured,
    } = effect
    else {
        return None;
    };
    let two_ranks = from.offset(0, 2) == Some(*to) || from.offset(0, -2) == Some(*to);
    if placed.is_foot_soldier() && captured.is_none() && two_ranks {
        Some((*from, *to, *placed))
    } else {
        None
    }
}

/// Returns `Some(landing)` when the just-double-stepped foot soldier (from `from`
/// to `to`) is legally capturable en passant next ply, otherwise `None`.
fn ep_flag_square(position: &Position, from: Square, to: Square, placed: Piece) -> Option<Square> {
    let landing = to;
    let skipped = if from.offset(0, 2) == Some(to) {
        from.offset(0, 1)
    } else {
        from.offset(0, -1)
    }?;

    let double_stepper_side = placed.side();
    let capturer_side = double_stepper_side.flip();
    let capturer_variant = position.variant_of(capturer_side);
    let double_stepper_variant = position.variant_of(double_stepper_side);

    // Tentative board: the victim is provisionally diminished, so that
    // `en_passant_capture` (which gates on the `-` marker) recognises it.
    let victim = position.piece_at(landing)?.epin();
    let victim_diminished = victim.with_pin(victim.pin().diminished());
    let tentative = |square: Square| {
        if square == landing {
            Some(Piece::new(victim_diminished))
        } else {
            position.piece_at(square)
        }
    };
    let real = |square: Square| position.piece_at(square);

    // Some opponent foot soldier must have a geometrically legal en passant
    // capture onto the skipped square whose victim is the landing piece, and that
    // capture must not leave the capturer's own royal in check.
    let legal = Square::all().any(|capturer_from| match position.piece_at(capturer_from) {
        Some(capturer) if capturer.is_foot_soldier() && capturer.belongs_to(capturer_side) => {
            en_passant_capture(
                capturer_variant,
                capturer_side,
                capturer_from,
                skipped,
                tentative,
            ) == Some(landing)
                && move_is_safe(
                    capturer_side,
                    double_stepper_variant,
                    Some(capturer_from),
                    skipped,
                    capturer,
                    Some(landing),
                    real,
                )
        }
        _ => false,
    });

    if legal {
        Some(landing)
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

    /// A straight two-rank foot-soldier advance effect.
    fn double_step(from: &str, to: &str, placed: &str) -> Effect {
        Effect::Board {
            from: sq(from),
            to: sq(to),
            placed: piece(placed),
            captured: None,
        }
    }

    #[test]
    fn sets_marker_when_capture_is_legal() {
        // White pawn just played e2→e4, adjacent black pawn on d4: e.p. capture
        // d4→e3 is legal. Black to move (the capturer).
        let p = pos("4k^3/8/8/8/3pP3/8/8/4K^3 / w/W");
        let after = recompose(&p, &double_step("e2", "e4", "P")).expect("recomposition");
        assert_eq!(
            token_at(&after, "e4"),
            Some("-P".to_owned()),
            "white pawn capturable e.p."
        );
    }

    #[test]
    fn no_marker_without_adjacent_capturer() {
        // No adjacent black pawn: no e.p. capture possible, hence no `-`.
        let p = pos("4k^3/8/8/8/4P3/8/8/4K^3 / w/W");
        let after = recompose(&p, &double_step("e2", "e4", "P")).expect("recomposition");
        assert_eq!(token_at(&after, "e4"), Some("P".to_owned()));
    }

    #[test]
    fn no_marker_if_capture_leaves_king_in_check() {
        // Black king a4, white rook h4: the e.p. capture d4→e3 would empty rank 4
        // and expose the black king to the rook. The capture is therefore illegal →
        // no marker (self-check condition).
        let p = pos("8/8/8/8/k^2pP2R/8/8/4K^3 / w/W");
        let after = recompose(&p, &double_step("e2", "e4", "P")).expect("recomposition");
        assert_eq!(
            token_at(&after, "e4"),
            Some("P".to_owned()),
            "e.p. capture illegal (self-check)"
        );
    }

    #[test]
    fn xiongqi_marker_via_sideways_capture() {
        // Red soldier f2→f4; black soldier on e3 (the skipped square's rank, having
        // crossed the river) captures sideways e3→f3. Black to move.
        let p = pos("7g^/8/8/8/5S2/4s3/8/G^7 / c/C");
        let after = recompose(&p, &double_step("f2", "f4", "S")).expect("recomposition");
        assert_eq!(
            token_at(&after, "f4"),
            Some("-S".to_owned()),
            "red soldier capturable e.p."
        );
    }

    #[test]
    fn clears_stale_marker() {
        // Residual `-P` from an earlier ply; this ply is a King move (not a double
        // step): the e.p. window has closed, the marker is removed.
        let p = pos("8/4k^3/8/8/4-P3/8/8/4K^3 / W/w");
        let king_move = Effect::Board {
            from: sq("e8"),
            to: sq("e7"),
            placed: piece("k^"),
            captured: None,
        };
        let after = recompose(&p, &king_move).expect("recomposition");
        assert_eq!(
            token_at(&after, "e4"),
            Some("P".to_owned()),
            "stale e.p. marker removed"
        );
    }
}
