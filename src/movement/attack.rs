//! Attack detection — the **heart of check detection**.
//!
//! `is_attacked` answers: "is the `target` square attacked by at least one piece
//! of side `by`?". The module is **decoupled from `Position`**: it receives a
//! `piece_at` closure and the attacker's variant, which makes it pure and
//! directly testable. Callers (check detection, castling legality, *uchifuzume*)
//! provide `|s| position.piece_at(s)`, `position.variant_of(by)` and `by`.
//!
//! The dispatch relies on each piece's **type letter** (canonical uppercase),
//! building on the already-verified geometric components:
//!
//! | Letter | Piece (variant) | Attack |
//! |--------|-----------------|--------|
//! | `K` | King (chess/ōgi) | one step (8 directions) |
//! | `G` | General (xiongqi) | orthogonal slide ("flying general") |
//! | `Q` | Queen (chess) | 8-direction slide |
//! | `R` | Rook / Chariot | orthogonal slide |
//! | `B` | Bishop / Bear | diagonal slide |
//! | `N` | Knight | L-leap |
//! | `I` | Princess (ōgi) | Bishop + Knight |
//! | `E` | Empress (xiongqi) | Chariot + Knight |
//! | `T` | Tokin (ōgi) | gold general (side-dependent) |
//! | `P`/`F`/`S` | Foot-soldier | variant-specific capture pattern |
//!
//! A letter outside this vocabulary attacks nothing (defensive branch).

use crate::domain::piece::Piece;
use crate::domain::side::Side;
use crate::domain::square::Square;
use crate::domain::variant::Variant;
use crate::movement::{foot_soldier, gold_directions, leaper, slider, stepper, Occupancy};

/// True if `target` is attacked by at least one piece of side `by` (of variant
/// `by_variant`) in the position described by `piece_at`.
#[must_use]
pub fn is_attacked(
    target: Square,
    by: Side,
    by_variant: Variant,
    piece_at: impl Fn(Square) -> Option<Piece>,
) -> bool {
    // Occupancy relative to the attacker: any piece, friendly or enemy, blocks a
    // slider (the target square itself is not tested by the components).
    let occupancy = |s: Square| Occupancy::of(piece_at(s).map(Piece::side), by);
    Square::all().any(|from| match piece_at(from) {
        Some(piece) if piece.belongs_to(by) => {
            piece_attacks(by_variant, piece, by, from, target, &occupancy)
        }
        _ => false,
    })
}

/// True if `piece` (side `side`, variant `variant`) placed on `from` attacks
/// `target`.
fn piece_attacks(
    variant: Variant,
    piece: Piece,
    side: Side,
    from: Square,
    target: Square,
    occupancy: &impl Fn(Square) -> Occupancy,
) -> bool {
    match piece.kind_letter() {
        // King (chess/ōgi): one step in the eight adjacent directions.
        'K' => stepper::attacks(from, &stepper::KING, target),
        // General (xiongqi): captures Chariot-style — this is what realizes the
        // "flying general" when the line between the two kings is clear.
        'G' => slider::attacks(from, &slider::ORTHOGONAL, target, occupancy),
        // Queen (chess).
        'Q' => slider::attacks(from, &slider::OCTILINEAR, target, occupancy),
        // Rook (chess/ōgi) / Chariot (xiongqi).
        'R' => slider::attacks(from, &slider::ORTHOGONAL, target, occupancy),
        // Bishop (chess/ōgi) / Bear (xiongqi).
        'B' => slider::attacks(from, &slider::DIAGONAL, target, occupancy),
        // Knight (all three variants, never blocked).
        'N' => leaper::attacks(from, &leaper::KNIGHT, target),
        // Princess (ōgi): Bishop + Knight.
        'I' => {
            slider::attacks(from, &slider::DIAGONAL, target, occupancy)
                || leaper::attacks(from, &leaper::KNIGHT, target)
        }
        // Empress (xiongqi): Chariot + Knight.
        'E' => {
            slider::attacks(from, &slider::ORTHOGONAL, target, occupancy)
                || leaper::attacks(from, &leaper::KNIGHT, target)
        }
        // Tokin (ōgi): gold general's move (one step, single source of the
        // directions).
        'T' => stepper::attacks(from, &gold_directions(side), target),
        // Foot-soldiers: Pawn, Fu, Soldier (variant-specific attack pattern).
        'P' | 'F' | 'S' => foot_soldier::attacks(variant, side, from, target),
        // Outside the vocabulary: attacks nothing.
        _ => false,
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

    use super::is_attacked;
    use crate::domain::piece::Piece;
    use crate::domain::side::Side;
    use crate::domain::square::Square;
    use crate::domain::variant::Variant;
    use sashite_epin::Identifier as Epin;

    fn sq(s: &str) -> Square {
        Square::parse(s).expect("valid square")
    }

    /// Builds a piece from its EPIN token (e.g. `"R"`, `"k^"`, `"+P"`).
    fn piece(token: &str) -> Piece {
        Piece::new(Epin::parse(token).expect("valid EPIN"))
    }

    /// Immutable board described by a `(square, token)` list.
    fn board<'a>(squares: &'a [(&'a str, &'a str)]) -> impl Fn(Square) -> Option<Piece> + 'a {
        move |s| {
            squares
                .iter()
                .find(|(name, _)| sq(name) == s)
                .map(|(_, tok)| piece(tok))
        }
    }

    #[test]
    fn rook_attacks_in_line_but_blocked() {
        // White rook on a1; target a8. Clear line -> attack.
        let b = board(&[("a1", "R")]);
        assert!(is_attacked(sq("a8"), Side::First, Variant::Chess, &b));
        // A blocker on a4 (whatever the side) cuts the attack beyond it.
        let b2 = board(&[("a1", "R"), ("a4", "p")]);
        assert!(!is_attacked(sq("a8"), Side::First, Variant::Chess, &b2));
        assert!(is_attacked(sq("a4"), Side::First, Variant::Chess, &b2)); // captures the blocker
    }

    #[test]
    fn knight_leaps_over() {
        let b = board(&[("d4", "N"), ("d5", "p"), ("d3", "p")]);
        assert!(is_attacked(sq("e6"), Side::First, Variant::Chess, &b));
        assert!(is_attacked(sq("f5"), Side::First, Variant::Chess, &b));
        assert!(!is_attacked(sq("d6"), Side::First, Variant::Chess, &b));
    }

    #[test]
    fn pawn_attacks_diagonally_not_straight() {
        let b = board(&[("e4", "P")]);
        assert!(is_attacked(sq("d5"), Side::First, Variant::Chess, &b));
        assert!(is_attacked(sq("f5"), Side::First, Variant::Chess, &b));
        assert!(!is_attacked(sq("e5"), Side::First, Variant::Chess, &b));
    }

    #[test]
    fn king_attacks_adjacent_squares() {
        let b = board(&[("e4", "K^")]);
        assert!(is_attacked(sq("e5"), Side::First, Variant::Chess, &b));
        assert!(is_attacked(sq("d3"), Side::First, Variant::Chess, &b));
        assert!(!is_attacked(sq("e6"), Side::First, Variant::Chess, &b));
    }

    #[test]
    fn flying_general_attacks_in_line() {
        // Xiongqi General: Chariot reach.
        let b = board(&[("e1", "G^")]);
        assert!(is_attacked(sq("e8"), Side::First, Variant::Xiongqi, &b));
        assert!(!is_attacked(sq("f2"), Side::First, Variant::Xiongqi, &b));
    }

    #[test]
    fn empress_combines_chariot_and_knight() {
        let b = board(&[("d4", "E")]);
        assert!(is_attacked(sq("d8"), Side::First, Variant::Xiongqi, &b)); // chariot
        assert!(is_attacked(sq("e6"), Side::First, Variant::Xiongqi, &b)); // knight
        assert!(!is_attacked(sq("f6"), Side::First, Variant::Xiongqi, &b)); // neither
    }

    #[test]
    fn princess_combines_bishop_and_knight() {
        let b = board(&[("d4", "I")]);
        assert!(is_attacked(sq("g7"), Side::First, Variant::Ogi, &b)); // bishop
        assert!(is_attacked(sq("e6"), Side::First, Variant::Ogi, &b)); // knight
        assert!(!is_attacked(sq("d8"), Side::First, Variant::Ogi, &b)); // not orthogonal
    }

    #[test]
    fn tokin_attacks_as_gold_general() {
        // First's Tokin on e4: orthogonals + forward diagonals (e5 on the +rank side).
        let b = board(&[("e4", "T")]);
        assert!(is_attacked(sq("e5"), Side::First, Variant::Ogi, &b)); // forward
        assert!(is_attacked(sq("d5"), Side::First, Variant::Ogi, &b)); // forward diagonal
        assert!(is_attacked(sq("e3"), Side::First, Variant::Ogi, &b)); // orthogonal backward
        assert!(!is_attacked(sq("d3"), Side::First, Variant::Ogi, &b)); // backward diagonal: no
    }

    #[test]
    fn only_the_attacking_side_counts() {
        // An enemy rook (lowercase) does not attack on First's behalf.
        let b = board(&[("a1", "r")]);
        assert!(!is_attacked(sq("a8"), Side::First, Variant::Chess, &b));
        assert!(is_attacked(sq("a8"), Side::Second, Variant::Chess, &b));
    }
}
