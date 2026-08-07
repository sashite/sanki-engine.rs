//! Attack detection — the **heart of check detection**.
//!
//! Three readings of one relation, from the coarsest to the finest:
//!
//! - [`is_attacked`] — "is `target` attacked by at least one piece of side
//!   `by`?". Short-circuits on the first attacker; this is the check test.
//! - [`attackers_of`] — *which* pieces of side `by` attack `target`. Scans the
//!   whole board, so it costs more than [`is_attacked`] and answers more: a
//!   caller that must count the attackers, or read their kinds, needs this one
//!   (a double check is two attackers, a smothered mate is one and it is a
//!   Knight).
//! - [`attacks_from`] — does the piece standing on `from` attack `target`. The
//!   binary relation the other two are built on, for a caller that already
//!   knows which piece it is asking about.
//!
//! All three ignore the occupant of `target`: a square is attacked whatever
//! stands on it, including a piece of the attacker's own side. That is what
//! makes [`attacks_from`] answer "does this piece **defend** that one".
//!
//! The module is **decoupled from `Position`**: it receives a `piece_at` closure
//! and the attacker's variant, which makes it pure and directly testable.
//! Callers (check detection, castling legality, *uchifuzume*) provide
//! `|s| position.piece_at(s)`, `position.variant_of(by)` and `by`.
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

/// The squares holding a piece of side `by` (of variant `by_variant`) that
/// attacks `target`, in board order.
///
/// The set [`is_attacked`] only reports the emptiness of. Use it where the
/// *number* of attackers or their *kinds* decide the answer; prefer
/// [`is_attacked`] where only "any at all" matters, since this one has no early
/// exit.
///
/// The occupant of `target` is not tested, so an attacker of a square held by
/// its own side is listed: `attackers_of(b, side, …).contains(&a)` reads as
/// "`a` defends `b`".
#[must_use]
pub fn attackers_of(
    target: Square,
    by: Side,
    by_variant: Variant,
    piece_at: impl Fn(Square) -> Option<Piece>,
) -> Vec<Square> {
    let occupancy = |s: Square| Occupancy::of(piece_at(s).map(Piece::side), by);
    Square::all()
        .filter(|&from| match piece_at(from) {
            Some(piece) if piece.belongs_to(by) => {
                piece_attacks(by_variant, piece, by, from, target, &occupancy)
            }
            _ => false,
        })
        .collect()
}

/// True if the piece standing on `from` attacks `target`; false if `from` is
/// empty.
///
/// `from_variant` MUST be the variant of the side the piece on `from` belongs
/// to — `position.variant_of(piece.side())`, not `position.active_variant()`.
/// The distinction is not cosmetic: a foot-soldier's attack pattern is the one
/// place the letter alone does not settle the geometry, so passing the other
/// camp's variant in a cross-variant position silently answers about a
/// different piece.
#[must_use]
pub fn attacks_from(
    from: Square,
    from_variant: Variant,
    target: Square,
    piece_at: impl Fn(Square) -> Option<Piece>,
) -> bool {
    match piece_at(from) {
        Some(piece) => {
            let side = piece.side();
            let occupancy = |s: Square| Occupancy::of(piece_at(s).map(Piece::side), side);
            piece_attacks(from_variant, piece, side, from, target, &occupancy)
        }
        None => false,
    }
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

    use super::{attackers_of, attacks_from, is_attacked};
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
    fn queen_attacks_all_eight_directions() {
        let b = board(&[("d4", "Q")]);
        assert!(is_attacked(sq("d8"), Side::First, Variant::Chess, &b)); // orthogonal
        assert!(is_attacked(sq("a1"), Side::First, Variant::Chess, &b)); // diagonal
        assert!(!is_attacked(sq("e6"), Side::First, Variant::Chess, &b)); // neither line
    }

    #[test]
    fn bishop_attacks_diagonally_only() {
        let b = board(&[("d4", "B")]);
        assert!(is_attacked(sq("a1"), Side::First, Variant::Chess, &b)); // diagonal
        assert!(is_attacked(sq("h8"), Side::First, Variant::Chess, &b)); // diagonal
        assert!(!is_attacked(sq("d8"), Side::First, Variant::Chess, &b)); // not orthogonal
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
    fn tokin_attacks_as_gold_general_second_side() {
        // Second's Tokin on e4: the vertical mirror — forward diagonals point
        // toward the lower ranks (d3, not d5).
        let b = board(&[("e4", "t")]);
        assert!(is_attacked(sq("e3"), Side::Second, Variant::Ogi, &b)); // forward
        assert!(is_attacked(sq("d3"), Side::Second, Variant::Ogi, &b)); // forward diagonal
        assert!(is_attacked(sq("e5"), Side::Second, Variant::Ogi, &b)); // orthogonal backward
        assert!(!is_attacked(sq("d5"), Side::Second, Variant::Ogi, &b)); // backward diagonal: no
    }

    #[test]
    fn only_the_attacking_side_counts() {
        // An enemy rook (lowercase) does not attack on First's behalf.
        let b = board(&[("a1", "r")]);
        assert!(!is_attacked(sq("a8"), Side::First, Variant::Chess, &b));
        assert!(is_attacked(sq("a8"), Side::Second, Variant::Chess, &b));
    }

    // --- attackers_of ------------------------------------------------------

    fn names(squares: &[Square]) -> Vec<String> {
        squares.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn attackers_of_lists_every_attacker_and_only_them() {
        // Two rooks bear on e4: Re1 up the file, Ra4 along the rank. Rh4 is cut
        // off by the enemy pawn on f4. Nf6 leaps in. The enemy rook on e8 is
        // not First's. The friendly bishop on b8 runs b8-f4, missing e4.
        let b = board(&[
            ("e1", "R"),
            ("a4", "R"),
            ("h4", "R"),
            ("f4", "p"),
            ("f6", "N"),
            ("b8", "B"),
            ("e8", "r"),
        ]);
        let found = attackers_of(sq("e4"), Side::First, Variant::Chess, &b);
        assert_eq!(names(&found), ["e1", "a4", "f6"]);
        assert!(is_attacked(sq("e4"), Side::First, Variant::Chess, &b));
        // The side filter cuts both ways: Second's own rook bears down the file,
        // and Second's pawn on f4 bears away from e4, not onto it.
        let theirs = attackers_of(sq("e4"), Side::Second, Variant::Chess, &b);
        assert_eq!(names(&theirs), ["e8"]);
    }

    #[test]
    fn attackers_of_counts_a_double_check() {
        // The discriminating case the count exists for: the royal on g8 is
        // attacked by the rook AND the knight, and by nothing else.
        let b = board(&[("g8", "k^"), ("g1", "R"), ("f6", "N"), ("a1", "K^")]);
        assert_eq!(
            attackers_of(sq("g8"), Side::First, Variant::Chess, &b).len(),
            2
        );
    }

    #[test]
    fn attackers_of_includes_a_defender_of_its_own_side() {
        // `defends` is this relation applied to an occupied square: the knight
        // on f6 attacks h7, where First's own rook stands.
        let b = board(&[("f6", "N"), ("h7", "R")]);
        let found = attackers_of(sq("h7"), Side::First, Variant::Chess, &b);
        assert_eq!(names(&found), ["f6"]);
    }

    #[test]
    fn attackers_of_is_empty_exactly_when_is_attacked_is_false() {
        // The invariant that keeps the two readings from ever disagreeing,
        // checked on every square of a mixed board.
        let b = board(&[
            ("d4", "Q"),
            ("g1", "N"),
            ("c2", "P"),
            ("e5", "p"),
            ("h8", "k^"),
            ("b1", "K^"),
        ]);
        for target in Square::all() {
            for side in [Side::First, Side::Second] {
                assert_eq!(
                    is_attacked(target, side, Variant::Chess, &b),
                    !attackers_of(target, side, Variant::Chess, &b).is_empty(),
                    "disagreement on {target} for {side:?}"
                );
            }
        }
    }

    #[test]
    fn attackers_of_reports_the_flying_general() {
        // Two Generals down an open file attack each other, which is what makes
        // the facing pair illegal rather than merely unusual.
        let b = board(&[("e1", "G^"), ("e8", "g^")]);
        assert_eq!(
            names(&attackers_of(sq("e8"), Side::First, Variant::Xiongqi, &b)),
            ["e1"]
        );
        assert_eq!(
            names(&attackers_of(sq("e1"), Side::Second, Variant::Xiongqi, &b)),
            ["e8"]
        );
    }

    // --- attacks_from ------------------------------------------------------

    #[test]
    fn attacks_from_is_the_per_piece_relation() {
        let b = board(&[("a1", "R"), ("a4", "p"), ("h1", "R")]);
        assert!(attacks_from(sq("a1"), Variant::Chess, sq("a4"), &b));
        assert!(!attacks_from(sq("a1"), Variant::Chess, sq("a8"), &b));
        // The other rook reaches a1 along the rank — nothing between them.
        assert!(attacks_from(sq("h1"), Variant::Chess, sq("a1"), &b));
    }

    #[test]
    fn attacks_from_an_empty_square_is_false() {
        let b = board(&[("a1", "R")]);
        assert!(!attacks_from(sq("d4"), Variant::Chess, sq("d8"), &b));
    }

    #[test]
    fn attacks_from_ignores_the_target_occupant() {
        // A piece defends its own side's piece, and attacks the enemy's, by the
        // same relation.
        let b = board(&[("d4", "B"), ("f6", "P"), ("b6", "p")]);
        assert!(attacks_from(sq("d4"), Variant::Chess, sq("f6"), &b));
        assert!(attacks_from(sq("d4"), Variant::Chess, sq("b6"), &b));
    }

    #[test]
    fn attacks_from_reads_the_variant_of_the_piece_on_the_square() {
        // The one geometry the letter does not settle. A chess Pawn on e4 bears
        // on the two forward diagonals; an ōgi Fu bears straight ahead. Same
        // square, same side, different variant, disjoint answers — which is why
        // the parameter is the variant of THAT piece's side.
        let pawn = board(&[("e4", "P")]);
        assert!(attacks_from(sq("e4"), Variant::Chess, sq("d5"), &pawn));
        assert!(!attacks_from(sq("e4"), Variant::Chess, sq("e5"), &pawn));

        let fu = board(&[("e4", "F")]);
        assert!(attacks_from(sq("e4"), Variant::Ogi, sq("e5"), &fu));
        assert!(!attacks_from(sq("e4"), Variant::Ogi, sq("d5"), &fu));
    }

    #[test]
    fn attacks_from_agrees_with_attackers_of_on_every_pair() {
        // The two public readings are one relation: for every ordered pair of
        // squares, membership in `attackers_of` is `attacks_from`.
        let b = board(&[
            ("d4", "I"),
            ("e6", "T"),
            ("c3", "F"),
            ("f2", "R"),
            ("g7", "b"),
            ("a8", "k^"),
            ("h1", "K^"),
        ]);
        for target in Square::all() {
            let listed = attackers_of(target, Side::First, Variant::Ogi, &b);
            for from in Square::all() {
                let owned = b(from).is_some_and(|p| p.belongs_to(Side::First));
                let direct = owned && attacks_from(from, Variant::Ogi, target, &b);
                assert_eq!(
                    listed.contains(&from),
                    direct,
                    "disagreement on {from} -> {target}"
                );
            }
        }
    }
}
