//! Foot-soldiers: the *variant- and side-dependent* movement of the Pawn
//! (chess), the Fu (ōgi) and the Soldier (xiongqi).
//!
//! "Forward" depends on the side: `first` advances toward the higher ranks
//! (rank delta `+1`), `second` toward the lower ranks (`-1`).
//!
//! | Variant | Advance | Double step (if `+`) | Capture |
//! |---------|---------|----------------------|---------|
//! | Pawn    | 1 straight (empty square) | 2 straight (two empty squares) | **diagonal** forward |
//! | Fu      | 1 straight (empty or capture) | 2 straight | straight |
//! | Soldier | 1 straight (empty or capture) | 2 straight | straight; **+ sideways** past the river |
//!
//! **En passant** capture is not produced here (it depends on the enemy's
//! `-P`/`-S` state and belongs to `legality::en_passant`). Likewise, promotion on
//! the last rank is handled by `legality`. River crossing (Soldier): `first` when
//! the internal rank ≥ 4 (board ranks 5–8), `second` ≤ 3 (board ranks 1–4).

use crate::domain::side::Side;
use crate::domain::square::Square;
use crate::domain::variant::Variant;
use crate::movement::{forward, Occupancy};

/// True if a Soldier of side `side` standing on `from` has crossed the river
/// (and thus gains the sideways step).
#[inline]
const fn crossed_river(side: Side, from: Square) -> bool {
    match side {
        Side::First => from.rank() >= 4,
        Side::Second => from.rank() <= 3,
    }
}

/// Squares reachable as a move by a foot-soldier (en passant excluded).
///
/// `enhanced` indicates eligibility for the double step (the `+` marker).
#[must_use]
pub fn destinations(
    variant: Variant,
    side: Side,
    from: Square,
    enhanced: bool,
    occupancy: impl Fn(Square) -> Occupancy,
) -> Vec<Square> {
    let fwd = forward(side);
    let mut out = Vec::new();
    match variant {
        Variant::Chess => {
            // Advance straight without capture; captures diagonally forward.
            forward_run(from, fwd, enhanced, false, &occupancy, &mut out);
            for df in [-1, 1] {
                if let Some(diag) = from.offset(df, fwd) {
                    if matches!(occupancy(diag), Occupancy::Enemy) {
                        out.push(diag);
                    }
                }
            }
        }
        Variant::Ogi => {
            // Advances and captures straight.
            forward_run(from, fwd, enhanced, true, &occupancy, &mut out);
        }
        Variant::Xiongqi => {
            // Advances and captures straight; sideways step/capture past the river.
            forward_run(from, fwd, enhanced, true, &occupancy, &mut out);
            if crossed_river(side, from) {
                for df in [-1, 1] {
                    if let Some(side_sq) = from.offset(df, 0) {
                        match occupancy(side_sq) {
                            Occupancy::Empty | Occupancy::Enemy => out.push(side_sq),
                            Occupancy::Friendly => {}
                        }
                    }
                }
            }
        }
    }
    out
}

/// Advances straight by one square (and by two if `enhanced` and both squares
/// are empty); captures straight only if `capture_straight`.
fn forward_run(
    from: Square,
    fwd: i8,
    enhanced: bool,
    capture_straight: bool,
    occupancy: &impl Fn(Square) -> Occupancy,
    out: &mut Vec<Square>,
) {
    let Some(step1) = from.offset(0, fwd) else {
        return;
    };
    match occupancy(step1) {
        Occupancy::Empty => {
            out.push(step1);
            if enhanced {
                if let Some(step2) = step1.offset(0, fwd) {
                    if matches!(occupancy(step2), Occupancy::Empty) {
                        out.push(step2);
                    }
                }
            }
        }
        Occupancy::Enemy => {
            if capture_straight {
                out.push(step1);
            }
        }
        Occupancy::Friendly => {}
    }
}

/// True if a foot-soldier of side `side` on `from` **attacks** `target` (capture
/// pattern, independent of occupancy since it is a single step).
#[must_use]
pub fn attacks(variant: Variant, side: Side, from: Square, target: Square) -> bool {
    let fwd = forward(side);
    match variant {
        // The Pawn attacks diagonally forward (even though it advances straight).
        Variant::Chess => hits(from, target, &[(-1, fwd), (1, fwd)]),
        // The Fu attacks straight (it captures straight).
        Variant::Ogi => hits(from, target, &[(0, fwd)]),
        // The Soldier attacks straight, and sideways past the river.
        Variant::Xiongqi => {
            hits(from, target, &[(0, fwd)])
                || (crossed_river(side, from) && hits(from, target, &[(-1, 0), (1, 0)]))
        }
    }
}

/// True if one of the offsets, applied to `from`, reaches `target` on the board.
#[inline]
fn hits(from: Square, target: Square, offsets: &[(i8, i8)]) -> bool {
    offsets
        .iter()
        .any(|&(df, dr)| from.offset(df, dr) == Some(target))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::{attacks, destinations};
    use crate::domain::side::Side;
    use crate::domain::square::Square;
    use crate::domain::variant::Variant;
    use crate::movement::Occupancy;

    fn sq(s: &str) -> Square {
        Square::parse(s).expect("valid square")
    }

    fn occ<'a>(pieces: &'a [(&'a str, Occupancy)]) -> impl Fn(Square) -> Occupancy + 'a {
        move |s| {
            pieces
                .iter()
                .find(|(name, _)| sq(name) == s)
                .map_or(Occupancy::Empty, |(_, o)| *o)
        }
    }

    fn empty(_: Square) -> Occupancy {
        Occupancy::Empty
    }

    // --- Pawn (chess) ---

    #[test]
    fn white_pawn_initial() {
        // e2, eligible for the double step, empty board.
        let d = destinations(Variant::Chess, Side::First, sq("e2"), true, empty);
        assert_eq!(d.len(), 2);
        assert!(d.contains(&sq("e3")));
        assert!(d.contains(&sq("e4")));
    }

    #[test]
    fn pawn_captures_diagonally_not_straight() {
        // Enemy ahead (e3) and diagonally (d3); friendly on f3.
        let board = occ(&[
            ("e3", Occupancy::Enemy),
            ("d3", Occupancy::Enemy),
            ("f3", Occupancy::Friendly),
        ]);
        let d = destinations(Variant::Chess, Side::First, sq("e2"), true, board);
        assert!(d.contains(&sq("d3")), "diagonal capture");
        assert!(!d.contains(&sq("e3")), "does not capture straight");
        assert!(!d.contains(&sq("e4")), "blocked, no double step");
        assert!(!d.contains(&sq("f3")), "friendly");
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn ineligible_pawn_no_double_step() {
        let d = destinations(Variant::Chess, Side::First, sq("e3"), false, empty);
        assert_eq!(d, vec![sq("e4")]);
    }

    #[test]
    fn black_pawn_advances_downward() {
        let d = destinations(Variant::Chess, Side::Second, sq("e7"), true, empty);
        assert!(d.contains(&sq("e6")));
        assert!(d.contains(&sq("e5")));
        assert_eq!(d.len(), 2);
    }

    // --- Fu (ōgi) ---

    #[test]
    fn fu_advances_and_captures_straight() {
        let d = destinations(Variant::Ogi, Side::First, sq("f4"), true, empty);
        assert_eq!(d.len(), 2); // f5, f6 (double)
        assert!(d.contains(&sq("f5")));
        assert!(d.contains(&sq("f6")));

        // Enemy straight ahead: capture, no double.
        let board = occ(&[("f5", Occupancy::Enemy)]);
        let d2 = destinations(Variant::Ogi, Side::First, sq("f4"), true, board);
        assert_eq!(d2, vec![sq("f5")]);
    }

    // --- Soldier (xiongqi) ---

    #[test]
    fn soldier_before_the_river_straight() {
        // e2 (internal rank 1): not past the river yet, no sideways.
        let d = destinations(Variant::Xiongqi, Side::First, sq("e2"), true, empty);
        assert_eq!(d.len(), 2); // e3, e4 (double)
        assert!(d.contains(&sq("e3")));
        assert!(d.contains(&sq("e4")));
    }

    #[test]
    fn soldier_past_the_river_gains_sideways() {
        // e5 (internal rank 4): river crossed; already moved (ineligible).
        let board = occ(&[("f5", Occupancy::Enemy)]);
        let d = destinations(Variant::Xiongqi, Side::First, sq("e5"), false, board);
        assert!(d.contains(&sq("e6")), "straight");
        assert!(d.contains(&sq("d5")), "left sideways (empty)");
        assert!(d.contains(&sq("f5")), "right sideways (capture)");
        assert_eq!(d.len(), 3);
    }

    // --- Attacks ---

    #[test]
    fn attacks_by_variant() {
        // Pawn: forward diagonals.
        assert!(attacks(Variant::Chess, Side::First, sq("e4"), sq("d5")));
        assert!(attacks(Variant::Chess, Side::First, sq("e4"), sq("f5")));
        assert!(!attacks(Variant::Chess, Side::First, sq("e4"), sq("e5")));

        // Fu: straight.
        assert!(attacks(Variant::Ogi, Side::First, sq("e4"), sq("e5")));
        assert!(!attacks(Variant::Ogi, Side::First, sq("e4"), sq("d5")));

        // Soldier before the river: straight only.
        assert!(attacks(Variant::Xiongqi, Side::First, sq("e2"), sq("e3")));
        assert!(!attacks(Variant::Xiongqi, Side::First, sq("e2"), sq("d2")));
        // Soldier past the river: straight + sideways.
        assert!(attacks(Variant::Xiongqi, Side::First, sq("e5"), sq("e6")));
        assert!(attacks(Variant::Xiongqi, Side::First, sq("e5"), sq("d5")));
        assert!(attacks(Variant::Xiongqi, Side::First, sq("e5"), sq("f5")));
    }
}
