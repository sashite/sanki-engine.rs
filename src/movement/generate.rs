//! Generation of **pseudo-legal moves** — the "movement" counterpart of
//! [`crate::movement::attack`].
//!
//! `pseudo_legal_destinations` returns the destination squares geometrically
//! reachable by a piece, accounting for blocking and captures, **but without**
//! the higher-order legality constraints: self-check, mandatory promotion, en
//! passant and drops are left to `legality`.
//!
//! The module is **decoupled from `Position`** and only needs the **occupancy**
//! of the other squares (the occupant's side, not its type): it takes an
//! `occupant_side` closure and thus stays pure and directly executable.
//!
//! Most pieces move as they attack; two notable deviations justify this module
//! being distinct from `attack`:
//!
//! - the **General** (xiongqi) is *asymmetric*: it advances one orthogonal step
//!   toward an **empty** square, but captures at **Chariot range**;
//! - the **foot-soldier** advances and captures differently (and benefits from
//!   the double step).

use crate::domain::side::Side;
use crate::domain::square::Square;
use crate::domain::variant::Variant;
use crate::movement::{foot_soldier, gold_directions, leaper, slider, stepper, Occupancy};

/// Pseudo-legal destination squares of a piece of type `kind_letter` (canonical
/// uppercase letter), of side `side`, placed on `from`.
///
/// `enhanced` carries eligibility for the double step (the `+` marker,
/// foot-soldiers). `occupant_side(s)` returns the side of the piece occupying
/// `s`, or `None` if the square is empty.
#[must_use]
pub fn pseudo_legal_destinations(
    variant: Variant,
    side: Side,
    kind_letter: char,
    enhanced: bool,
    from: Square,
    occupant_side: impl Fn(Square) -> Option<Side>,
) -> Vec<Square> {
    let occ = |s: Square| Occupancy::of(occupant_side(s), side);
    match kind_letter {
        // King (chess/ōgi): one step in the eight directions, captures included.
        'K' => stepper::destinations(from, &stepper::KING, true, occ),
        // General (xiongqi): orthogonal step toward empty + Chariot capture.
        'G' => general(from, &occ),
        // Queen (chess).
        'Q' => slider::destinations(from, &slider::OCTILINEAR, occ),
        // Rook (chess/ōgi) / Chariot (xiongqi).
        'R' => slider::destinations(from, &slider::ORTHOGONAL, occ),
        // Bishop (chess/ōgi) / Bear (xiongqi).
        'B' => slider::destinations(from, &slider::DIAGONAL, occ),
        // Knight.
        'N' => leaper::destinations(from, &leaper::KNIGHT, occ),
        // Princess (ōgi): Bishop + Knight (geometrically disjoint patterns).
        'I' => {
            let mut out = slider::destinations(from, &slider::DIAGONAL, occ);
            out.extend(leaper::destinations(from, &leaper::KNIGHT, occ));
            out
        }
        // Empress (xiongqi): Chariot + Knight (disjoint patterns).
        'E' => {
            let mut out = slider::destinations(from, &slider::ORTHOGONAL, occ);
            out.extend(leaper::destinations(from, &leaper::KNIGHT, occ));
            out
        }
        // Tokin (ōgi): one step in the gold general's six directions, captures
        // included.
        'T' => stepper::destinations(from, &gold_directions(side), true, occ),
        // Foot-soldiers: Pawn, Fu, Soldier.
        'P' | 'F' | 'S' => foot_soldier::destinations(variant, side, from, enhanced, occ),
        // Outside the vocabulary: no move.
        _ => Vec::new(),
    }
}

/// General's moves (xiongqi): move/capture asymmetry.
///
/// - Quiet move: an adjacent orthogonal **empty** square.
/// - Capture: Chariot slide, keeping only the first enemy piece on each ray
///   (which realizes the "flying general" on an open line).
fn general(from: Square, occ: &impl Fn(Square) -> Occupancy) -> Vec<Square> {
    let mut out = stepper::destinations(from, &slider::ORTHOGONAL, false, occ);
    for square in slider::destinations(from, &slider::ORTHOGONAL, occ) {
        if matches!(occ(square), Occupancy::Enemy) {
            out.push(square);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::pseudo_legal_destinations as gen;
    use crate::domain::side::Side;
    use crate::domain::square::Square;
    use crate::domain::variant::Variant;

    fn sq(s: &str) -> Square {
        Square::parse(s).expect("valid square")
    }

    /// Board: a `(square, occupant_side)` list.
    fn board<'a>(squares: &'a [(&'a str, Side)]) -> impl Fn(Square) -> Option<Side> + 'a {
        move |s| {
            squares
                .iter()
                .find(|(name, _)| sq(name) == s)
                .map(|(_, side)| *side)
        }
    }

    fn empty(_: Square) -> Option<Side> {
        None
    }

    fn set(squares: &[&str]) -> std::collections::BTreeSet<Square> {
        squares.iter().map(|s| sq(s)).collect()
    }

    fn got(v: Vec<Square>) -> std::collections::BTreeSet<Square> {
        v.into_iter().collect()
    }

    #[test]
    fn rook_slides_until_blocked() {
        // First's Rook on a1; friendly on a4, enemy on e1.
        let b = board(&[("a4", Side::First), ("e1", Side::Second)]);
        let d = got(gen(Variant::Chess, Side::First, 'R', false, sq("a1"), &b));
        // File a: a2, a3 (a4 = friendly, blocks without capture).
        assert!(d.contains(&sq("a2")) && d.contains(&sq("a3")));
        assert!(!d.contains(&sq("a4")) && !d.contains(&sq("a5")));
        // Rank 1: b1..d1 then capturing e1, nothing beyond.
        assert!(d.contains(&sq("d1")) && d.contains(&sq("e1")));
        assert!(!d.contains(&sq("f1")));
    }

    #[test]
    fn king_one_step_all_directions_with_capture() {
        // First's King on e4; friendly on d4 (blocks), enemy on e5 (captured).
        let b = board(&[("d4", Side::First), ("e5", Side::Second)]);
        let d = got(gen(Variant::Chess, Side::First, 'K', false, sq("e4"), &b));
        assert_eq!(
            d,
            set(&["f4", "e5", "e3", "f5", "f3", "d5", "d3"]),
            "eight adjacent squares minus the friendly-occupied d4, capturing e5"
        );
    }

    #[test]
    fn queen_slides_all_eight_directions() {
        // First's Queen on d4; friendly on d6 (blocks), enemy on f4 (captured).
        let b = board(&[("d6", Side::First), ("f4", Side::Second)]);
        let d = got(gen(Variant::Chess, Side::First, 'Q', false, sq("d4"), &b));
        // Orthogonal: blocked north (d5 only); captures east (through e4 to f4).
        assert!(d.contains(&sq("d5")) && !d.contains(&sq("d6")) && !d.contains(&sq("d7")));
        assert!(d.contains(&sq("e4")) && d.contains(&sq("f4")) && !d.contains(&sq("g4")));
        // Diagonal reach both ways (a Rook alone would not have this).
        assert!(d.contains(&sq("a1")), "diagonal reach");
        assert!(d.contains(&sq("a7")), "other diagonal reach");
    }

    #[test]
    fn bishop_slides_diagonally_only() {
        let d = got(gen(
            Variant::Chess,
            Side::First,
            'B',
            false,
            sq("d4"),
            empty,
        ));
        assert_eq!(d.len(), 13);
        assert!(d.contains(&sq("a1")));
        assert!(d.contains(&sq("h8")));
        assert!(d.contains(&sq("a7")));
        assert!(d.contains(&sq("g1")));
        assert!(!d.contains(&sq("d8")), "not orthogonal");
    }

    #[test]
    fn knight_leaps_l_shape() {
        // First's Knight on d4; friendly on e6 (blocks), enemy on f5 (captured).
        let b = board(&[("e6", Side::First), ("f5", Side::Second)]);
        let d = got(gen(Variant::Chess, Side::First, 'N', false, sq("d4"), &b));
        assert_eq!(
            d,
            set(&["f5", "f3", "e2", "c2", "b3", "b5", "c6"]),
            "eight L-leaps minus the friendly-occupied e6, capturing f5"
        );
    }

    #[test]
    fn general_quiet_step_and_chariot_capture() {
        // First's General on e4; distant enemy on e7, adjacent friendly on d4.
        let b = board(&[("e7", Side::Second), ("d4", Side::First)]);
        let d = got(gen(Variant::Xiongqi, Side::First, 'G', false, sq("e4"), &b));
        // Quiet orthogonal step toward empty: e5, e3, f4 (d4 occupied by friendly).
        assert!(d.contains(&sq("e5")) && d.contains(&sq("e3")) && d.contains(&sq("f4")));
        assert!(!d.contains(&sq("d4")), "friendly square");
        // But NO quiet slide beyond one step:
        assert!(!d.contains(&sq("e6")), "no multi-square move");
        // Chariot capture, at distance: e7.
        assert!(d.contains(&sq("e7")), "long capture (flying general)");
    }

    #[test]
    fn general_adjacent_capture() {
        // Orthogonally adjacent enemy: captured by the slide (distance 1).
        let b = board(&[("e5", Side::Second)]);
        let d = got(gen(Variant::Xiongqi, Side::First, 'G', false, sq("e4"), &b));
        assert!(d.contains(&sq("e5")), "adjacent capture");
    }

    #[test]
    fn empress_chariot_plus_knight() {
        let d = got(gen(
            Variant::Xiongqi,
            Side::First,
            'E',
            false,
            sq("d4"),
            empty,
        ));
        assert!(d.contains(&sq("d8")), "chariot");
        assert!(d.contains(&sq("h4")), "chariot");
        assert!(d.contains(&sq("e6")), "knight");
        assert!(d.contains(&sq("f5")), "knight");
        assert!(!d.contains(&sq("f6")), "neither");
    }

    #[test]
    fn princess_bishop_plus_knight() {
        // Ōgi's Princess on d4: Bishop's diagonal slide plus Knight's leap.
        let d = got(gen(Variant::Ogi, Side::First, 'I', false, sq("d4"), empty));
        assert!(d.contains(&sq("a1")), "bishop");
        assert!(d.contains(&sq("h8")), "bishop");
        assert!(d.contains(&sq("e6")), "knight");
        assert!(d.contains(&sq("f5")), "knight");
        assert!(!d.contains(&sq("d8")), "neither");
    }

    #[test]
    fn tokin_six_directions_no_backward_diagonal() {
        // First's Tokin on e4: 4 orthogonals + 2 forward diagonals.
        let d = got(gen(Variant::Ogi, Side::First, 'T', false, sq("e4"), empty));
        assert_eq!(
            d,
            set(&["e5", "e3", "d4", "f4", "d5", "f5"]),
            "exactly the gold general"
        );
    }

    #[test]
    fn tokin_six_directions_no_backward_diagonal_second_side() {
        // Second's Tokin on e4: the vertical mirror — forward diagonals point
        // toward the lower ranks (d3/f3), not the higher ones (d5/f5).
        let d = got(gen(Variant::Ogi, Side::Second, 'T', false, sq("e4"), empty));
        assert_eq!(
            d,
            set(&["e5", "e3", "d4", "f4", "d3", "f3"]),
            "exactly the gold general, mirrored"
        );
    }

    #[test]
    fn foot_soldier_delegates_with_double_step() {
        // First's Pawn on e2, eligible: e3 + e4.
        let d = got(gen(Variant::Chess, Side::First, 'P', true, sq("e2"), empty));
        assert_eq!(d, set(&["e3", "e4"]));
        // Ineligible: only e3.
        let d2 = got(gen(
            Variant::Chess,
            Side::First,
            'P',
            false,
            sq("e2"),
            empty,
        ));
        assert_eq!(d2, set(&["e3"]));
    }

    #[test]
    fn unknown_letter_no_move() {
        let d = gen(Variant::Chess, Side::First, 'Z', false, sq("e4"), empty);
        assert!(d.is_empty());
    }
}
