//! En passant — detecting and resolving the captured square.
//!
//! En passant is **asymmetric** depending on the **capturer**'s variant:
//!
//! - **Pawn** (chess): captures **diagonally forward** onto the skipped square;
//! - **Soldier** (xiongqi) having crossed the river: captures **sideways** onto
//!   the skipped square;
//! - **Fu** (ōgi): **never** captures en passant (it only captures straight).
//!
//! The **victim** is any enemy foot-soldier that just played the double step,
//! hence carrying the diminished `-` marker (`-P`, `-S` or — in cross-variant
//! play — `-F`). Since both double steps advance straight, the victim's square is
//! deduced uniformly: one rank **beyond** the destination square, in the
//! opponent's marching direction — `to.offset(0, forward(side.flip()))`.
//!
//! This module reads only the **geometry** and the **markers**; royal safety (the
//! capture must not expose one's own royal) is checked separately by
//! [`crate::legality::self_check`]. The `-` marker, moreover, is only set at
//! canonicalization when the capture is actually legal.

use crate::domain::piece::Piece;
use crate::domain::side::Side;
use crate::domain::square::Square;
use crate::domain::variant::Variant;
use crate::movement::forward;

/// If side `side`'s foot-soldier move `from -> to` (variant `variant`) is a legal
/// en-passant capture, returns the square of the **captured** foot-soldier
/// (distinct from `to`); otherwise `None`.
///
/// Conditions checked: `from` indeed carries a foot-soldier of `side`; `to` is
/// the skipped (empty) square; the geometry matches the capturer's mechanism; and
/// the square behind `to` carries a **diminished enemy** foot-soldier.
#[must_use]
pub fn en_passant_capture(
    variant: Variant,
    side: Side,
    from: Square,
    to: Square,
    piece_at: impl Fn(Square) -> Option<Piece>,
) -> Option<Square> {
    // The capturer must be a foot-soldier of `side`.
    match piece_at(from) {
        Some(mover) if mover.is_foot_soldier() && mover.belongs_to(side) => {}
        _ => return None,
    }
    // The capture occurs on the skipped square, which is empty.
    if piece_at(to).is_some() {
        return None;
    }
    // Geometry specific to the capturer's mechanism.
    let geometry_ok = match variant {
        // The ōgi Fu never captures en passant.
        Variant::Ogi => false,
        // Pawn: one diagonal forward step.
        Variant::Chess => is_diagonal_forward_step(side, from, to),
        // Soldier: one sideways step, past the river. On any position the
        // engine itself produces the crossing is automatic (a `-` victim only
        // exists on double-step landing ranks, whose adjacent files are past
        // the river for the capturer) — the explicit gate is defense in depth
        // against crafted inputs (rules-of-xiongqi §Illegal Moves: no sideways
        // step before the river).
        Variant::Xiongqi => {
            is_sideways_step(from, to) && crate::movement::foot_soldier::crossed_river(side, from)
        }
    };
    if !geometry_ok {
        return None;
    }
    // The victim sits one rank beyond `to`, in the opponent's marching direction,
    // and must be a diminished enemy foot-soldier.
    let captured = to.offset(0, forward(side.flip()))?;
    match piece_at(captured) {
        Some(victim)
            if victim.belongs_to(side.flip())
                && victim.is_foot_soldier()
                && victim.is_diminished() =>
        {
            Some(captured)
        }
        _ => None,
    }
}

/// True if `to` is reached from `from` by a diagonal forward step (the Pawn's
/// mechanism).
#[inline]
fn is_diagonal_forward_step(side: Side, from: Square, to: Square) -> bool {
    let fwd = forward(side);
    from.offset(-1, fwd) == Some(to) || from.offset(1, fwd) == Some(to)
}

/// True if `to` is reached from `from` by a one-square sideways step (the
/// Soldier's mechanism).
#[inline]
fn is_sideways_step(from: Square, to: Square) -> bool {
    from.offset(-1, 0) == Some(to) || from.offset(1, 0) == Some(to)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::en_passant_capture;
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

    #[test]
    fn chess_pawn_diagonal_capture() {
        // White pawn e5; black pawn d5 having just played d7->d5 (`-p`).
        let b = board(&[("e5", "P"), ("d5", "-p")]);
        assert_eq!(
            en_passant_capture(Variant::Chess, Side::First, sq("e5"), sq("d6"), &b),
            Some(sq("d5"))
        );
    }

    #[test]
    fn chess_pawn_without_marker_refused() {
        // The black pawn is not diminished: no en-passant right.
        let b = board(&[("e5", "P"), ("d5", "p")]);
        assert_eq!(
            en_passant_capture(Variant::Chess, Side::First, sq("e5"), sq("d6"), &b),
            None
        );
    }

    #[test]
    fn chess_pawn_occupied_destination_refused() {
        // d6 occupied -> this would be an ordinary capture, not en passant.
        let b = board(&[("e5", "P"), ("d5", "-p"), ("d6", "n")]);
        assert_eq!(
            en_passant_capture(Variant::Chess, Side::First, sq("e5"), sq("d6"), &b),
            None
        );
    }

    #[test]
    fn xiongqi_soldier_sideways_capture() {
        // Red soldier g6; black soldier f5 having played f7->f5 (`-s`), skipped square f6.
        let b = board(&[("g6", "S"), ("f5", "-s")]);
        assert_eq!(
            en_passant_capture(Variant::Xiongqi, Side::First, sq("g6"), sq("f6"), &b),
            Some(sq("f5"))
        );
        // From the other adjacent file (e6): same victim.
        let b2 = board(&[("e6", "S"), ("f5", "-s")]);
        assert_eq!(
            en_passant_capture(Variant::Xiongqi, Side::First, sq("e6"), sq("f6"), &b2),
            Some(sq("f5"))
        );
    }

    #[test]
    fn cross_chess_pawn_takes_ogi_fu() {
        // White chess pawn e5 takes a diminished black ōgi Fu d5 (`-f`).
        let b = board(&[("e5", "P"), ("d5", "-f")]);
        assert_eq!(
            en_passant_capture(Variant::Chess, Side::First, sq("e5"), sq("d6"), &b),
            Some(sq("d5"))
        );
    }

    #[test]
    fn cross_xiongqi_soldier_takes_chess_pawn() {
        // Red xiongqi soldier g6 takes a diminished black chess pawn f5 (`-p`).
        let b = board(&[("g6", "S"), ("f5", "-p")]);
        assert_eq!(
            en_passant_capture(Variant::Xiongqi, Side::First, sq("g6"), sq("f6"), &b),
            Some(sq("f5"))
        );
    }

    #[test]
    fn xiongqi_pre_river_sideways_capture_refused() {
        // Defense in depth: a crafted `-p` victim on a low rank must not grant a
        // PRE-river Soldier the sideways capture (rules-of-xiongqi §Illegal
        // Moves — no sideways step before the river). g3 is before First's
        // river (ranks 5+); the same geometry past the river is accepted.
        let b = board(&[("g3", "S"), ("f2", "-p")]);
        assert_eq!(
            en_passant_capture(Variant::Xiongqi, Side::First, sq("g3"), sq("f3"), &b),
            None
        );
    }

    #[test]
    fn ogi_fu_never_captures() {
        // Same geometric configuration as a Pawn capture: refused in ōgi.
        let b = board(&[("e5", "F"), ("d5", "-p")]);
        assert_eq!(
            en_passant_capture(Variant::Ogi, Side::First, sq("e5"), sq("d6"), &b),
            None
        );
    }

    #[test]
    fn non_foot_soldier_capturer_refused() {
        // A Rook on e5 does not capture en passant, whatever the geometry.
        let b = board(&[("e5", "R"), ("d5", "-p")]);
        assert_eq!(
            en_passant_capture(Variant::Chess, Side::First, sq("e5"), sq("d6"), &b),
            None
        );
    }

    #[test]
    fn second_player_diagonal_capture() {
        // Symmetry: black pawn d4 takes a white pawn e4 (`-P`) on square e3.
        let b = board(&[("d4", "p"), ("e4", "-P")]);
        assert_eq!(
            en_passant_capture(Variant::Chess, Side::Second, sq("d4"), sq("e3"), &b),
            Some(sq("e4"))
        );
    }
}
