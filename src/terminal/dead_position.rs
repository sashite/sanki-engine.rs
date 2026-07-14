//! Draw by **dead position** (termination status `insufficient`).
//!
//! FIDE dead-position rule, shared verbatim by the three rule specifications
//! (rules-of-*.md §End of Game): the game is an **immediate draw** as soon as
//! **neither player can checkmate the opponent's royal by any series of legal
//! moves**. The criterion is *possibility*, not *forceability*: a mate that
//! could only be reached with the opponent's cooperation still counts as
//! possible — thus King + two Knights versus a lone King is **not** dead.
//!
//! The engine detects the **material-only** configurations; dead positions that
//! depend on the arrangement of the pieces (e.g. fully blocked structures) are
//! not detected and reach a draw through repetition or the move limit instead.
//! The detected set is **variant-specific** (rules-of-*.md §Dead-Position
//! Detection):
//!
//! - **Pure chess** — King versus King; King + Bishop versus King; King +
//!   Knight versus King; Kings and Bishops only (any number, on either side)
//!   with all Bishops standing on squares of the same colour. Pieces in hand
//!   are ignored: the chess hand is inert and can never influence mating
//!   potential.
//! - **Pure ōgi** — **nothing**: captures never remove material from play (a
//!   captured piece is demoted, side-flipped, and stays droppable), so the
//!   total material in play is invariant and no materially dead configuration
//!   is reachable from the standard initial position. No detection is
//!   performed — this is deliberate; should non-standard start positions ever
//!   become possible (handicaps, composed positions), revisit this choice.
//! - **Pure xiongqi** — General versus General, the *only* material-only dead
//!   configuration (the General captures at Chariot range, so almost any extra
//!   material suffices to build a cooperative mate). The inert hand is
//!   ignored.
//! - **Mixed pairings** (cross-variant sessions) are governed by the
//!   interactions-*.md documents, unchanged: an immediate draw once only the
//!   two royals remain on the board and no hand holds a **droppable** piece
//!   (droppable iff it bears its holder's case — an inert piece from a
//!   chess/xiongqi capture does not prevent the draw). This deliberately
//!   covers the mixed King-versus-General pair: strictly not a dead position
//!   (a cornered General is matable in principle), but neither side can
//!   *force* a win, so Sashité declares the draw outright rather than making
//!   the players shuffle to a repetition or move-limit finish (deciders'
//!   ruling, 2026-07-10).
//!
//! The termination status keeps the protocol token `insufficient`
//! (statuses-sanki.md); "dead position" is the rule-level name.

use crate::domain::piece::Piece;
use crate::domain::side::Side;
use crate::domain::square::Square;
use crate::domain::variant::{Variant, VariantAssignment};

/// True if the position is a dead-position draw for the session's variant
/// pairing (see the module docs for the per-pairing detected sets).
///
/// `first_hand` / `second_hand` enumerate each player's pieces in hand; they
/// are only consulted for **mixed** pairings (a piece is droppable there iff
/// it belongs to the holding side). Pure-chess and pure-xiongqi hands are
/// inert by construction and ignored; pure ōgi performs no detection at all.
#[must_use]
pub fn is_dead_position(
    variants: VariantAssignment,
    piece_at: impl Fn(Square) -> Option<Piece>,
    first_hand: &[Piece],
    second_hand: &[Piece],
) -> bool {
    match (variants.first, variants.second) {
        // Pure chess: the four material-only configurations, hands ignored.
        (Variant::Chess, Variant::Chess) => chess_material_is_dead(&piece_at),
        // Pure ōgi: no materially dead configuration is reachable — nothing to
        // detect (rules-of-ogi.md §Dead-Position Detection).
        (Variant::Ogi, Variant::Ogi) => false,
        // Pure xiongqi: General versus General, the inert hand ignored.
        (Variant::Xiongqi, Variant::Xiongqi) => board_holds_only_royals(&piece_at),
        // Mixed pairings (interactions-*.md): only royals on the board and no
        // droppable material in hand.
        _ => board_holds_only_royals(&piece_at) && !any_droppable(first_hand, second_hand),
    }
}

/// Pure-chess material-only dead positions: no non-royal piece other than
/// Bishops and Knights, and then either **no Knight** with **all Bishops on
/// squares of the same colour** (covers King versus King and King + Bishop
/// versus King), or a **single Knight** and nothing else.
fn chess_material_is_dead(piece_at: &impl Fn(Square) -> Option<Piece>) -> bool {
    let mut knights: u8 = 0;
    // Bishops bucketed by square colour (the two diagonal-parity classes).
    let mut bishops: [u8; 2] = [0, 0];

    for square in Square::all() {
        let Some(piece) = piece_at(square) else {
            continue;
        };
        if piece.is_royal() {
            continue;
        }
        match piece.kind_letter() {
            'N' => knights = knights.saturating_add(1),
            'B' => {
                if square_colour(square) == 0 {
                    bishops[0] = bishops[0].saturating_add(1);
                } else {
                    bishops[1] = bishops[1].saturating_add(1);
                }
            }
            // Any other piece mates (Queen, Rook, or a Pawn that promotes).
            _ => return false,
        }
    }

    match (knights, bishops) {
        // Kings and Bishops only, all on the same colour — includes King
        // versus King (no Bishop at all) and King + Bishop versus King.
        (0, [_, 0]) | (0, [0, _]) => true,
        // King + Knight versus King. Two Knights (either split) are NOT dead:
        // a cooperative mate exists, and the game continues.
        (1, [0, 0]) => true,
        _ => false,
    }
}

/// The square's colour class, `0` or `1` (diagonal parity: `file + rank`
/// modulo 2, computed bitwise).
const fn square_colour(square: Square) -> u8 {
    (square.file() ^ square.rank()) & 1
}

/// True if every piece present on the board is a royal.
fn board_holds_only_royals(piece_at: &impl Fn(Square) -> Option<Piece>) -> bool {
    Square::all().all(|square| match piece_at(square) {
        Some(piece) => piece.is_royal(),
        None => true,
    })
}

/// True if at least one hand holds a piece droppable by its holder.
fn any_droppable(first_hand: &[Piece], second_hand: &[Piece]) -> bool {
    first_hand.iter().any(|p| p.belongs_to(Side::First))
        || second_hand.iter().any(|p| p.belongs_to(Side::Second))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::is_dead_position;
    use crate::domain::piece::Piece;
    use crate::domain::square::Square;
    use crate::domain::variant::{Variant, VariantAssignment};
    use sashite_epin::Identifier as Epin;

    const CHESS: VariantAssignment = VariantAssignment {
        first: Variant::Chess,
        second: Variant::Chess,
    };
    const OGI: VariantAssignment = VariantAssignment {
        first: Variant::Ogi,
        second: Variant::Ogi,
    };
    const XIONGQI: VariantAssignment = VariantAssignment {
        first: Variant::Xiongqi,
        second: Variant::Xiongqi,
    };
    const CHESS_V_XIONGQI: VariantAssignment = VariantAssignment {
        first: Variant::Chess,
        second: Variant::Xiongqi,
    };
    const OGI_V_XIONGQI: VariantAssignment = VariantAssignment {
        first: Variant::Ogi,
        second: Variant::Xiongqi,
    };
    const CHESS_V_OGI: VariantAssignment = VariantAssignment {
        first: Variant::Chess,
        second: Variant::Ogi,
    };

    fn sq(s: &str) -> Square {
        Square::parse(s).expect("valid square")
    }

    fn piece(token: &str) -> Piece {
        Piece::new(Epin::parse(token).expect("valid EPIN"))
    }

    fn board<'a>(cases: &'a [(&'a str, &'a str)]) -> impl Fn(Square) -> Option<Piece> + 'a {
        move |s| {
            cases
                .iter()
                .find(|(name, _)| sq(name) == s)
                .map(|(_, tok)| piece(tok))
        }
    }

    // ── Pure chess: the four material-only configurations ────────────────────

    #[test]
    fn chess_two_lone_kings() {
        let b = board(&[("e1", "K^"), ("e8", "k^")]);
        assert!(is_dead_position(CHESS, &b, &[], &[]));
    }

    #[test]
    fn chess_king_and_bishop_versus_king() {
        let b = board(&[("e1", "K^"), ("e8", "k^"), ("h1", "B")]);
        assert!(is_dead_position(CHESS, &b, &[], &[]));
        // The second player's Bishop draws just the same.
        let b2 = board(&[("e1", "K^"), ("e8", "k^"), ("c8", "b")]);
        assert!(is_dead_position(CHESS, &b2, &[], &[]));
    }

    #[test]
    fn chess_king_and_knight_versus_king() {
        let b = board(&[("e1", "K^"), ("e8", "k^"), ("g1", "N")]);
        assert!(is_dead_position(CHESS, &b, &[], &[]));
        let b2 = board(&[("e1", "K^"), ("e8", "k^"), ("b8", "n")]);
        assert!(is_dead_position(CHESS, &b2, &[], &[]));
    }

    #[test]
    fn chess_bishops_all_on_the_same_colour() {
        // h1 (7,0), c1? — pick three same-parity squares across both sides:
        // h1 (7^0=1), c8 (2^7=1), f1 (5^0=1). All colour 1.
        let b = board(&[
            ("e1", "K^"),
            ("e8", "k^"),
            ("h1", "B"),
            ("f1", "B"),
            ("c8", "b"),
        ]);
        assert!(is_dead_position(CHESS, &b, &[], &[]));
    }

    #[test]
    fn chess_opposite_coloured_bishops_are_not_dead() {
        // h1 is colour 1, b8 (1^7=0) is colour 0: a cooperative mate exists.
        let b = board(&[("e1", "K^"), ("e8", "k^"), ("h1", "B"), ("b8", "b")]);
        assert!(!is_dead_position(CHESS, &b, &[], &[]));
    }

    #[test]
    fn chess_two_knights_are_not_dead() {
        // Possibility, not forceability: K + N + N versus K continues.
        let b = board(&[("e1", "K^"), ("e8", "k^"), ("g1", "N"), ("h1", "N")]);
        assert!(!is_dead_position(CHESS, &b, &[], &[]));
        // A Knight on each side is not dead either.
        let b2 = board(&[("e1", "K^"), ("e8", "k^"), ("g1", "N"), ("b8", "n")]);
        assert!(!is_dead_position(CHESS, &b2, &[], &[]));
    }

    #[test]
    fn chess_knight_plus_bishop_is_not_dead() {
        let b = board(&[("e1", "K^"), ("e8", "k^"), ("g1", "N"), ("h1", "B")]);
        assert!(!is_dead_position(CHESS, &b, &[], &[]));
    }

    #[test]
    fn chess_any_other_piece_is_sufficient() {
        for token in ["Q", "R", "P", "q", "r", "p"] {
            let cases = [("e1", "K^"), ("e8", "k^"), ("a4", token)];
            let b = board(&cases);
            assert!(!is_dead_position(CHESS, &b, &[], &[]), "{token}");
        }
    }

    #[test]
    fn chess_hands_are_ignored() {
        // The chess hand is inert; even own-case material in hand can never
        // come back to the board (drops are ōgi-only).
        let b = board(&[("e1", "K^"), ("e8", "k^"), ("h1", "B")]);
        assert!(is_dead_position(
            CHESS,
            &b,
            &[piece("p"), piece("R")],
            &[piece("P")]
        ));
    }

    // ── Pure ōgi: no detection at all ─────────────────────────────────────────

    #[test]
    fn ogi_performs_no_detection() {
        // Even the materially dead lone-Kings-with-empty-hands configuration
        // (unreachable from the standard start) is NOT declared: pure ōgi has
        // no dead-position detection by specification.
        let b = board(&[("e1", "K^"), ("e8", "k^")]);
        assert!(!is_dead_position(OGI, &b, &[], &[]));
        // A fortiori with any material anywhere.
        let b2 = board(&[("e1", "K^"), ("e8", "k^"), ("h1", "B")]);
        assert!(!is_dead_position(OGI, &b2, &[], &[]));
    }

    // ── Pure xiongqi: General versus General only ─────────────────────────────

    #[test]
    fn xiongqi_general_versus_general() {
        let g = board(&[("e1", "G^"), ("e8", "g^")]);
        assert!(is_dead_position(XIONGQI, &g, &[], &[]));
        // The inert hand is ignored.
        assert!(is_dead_position(XIONGQI, &g, &[piece("s")], &[piece("S")]));
    }

    #[test]
    fn xiongqi_any_extra_material_is_sufficient() {
        // General + Bear mates, General + Knight mates, a Soldier promotes to
        // an Empress: no other material class is dead.
        for token in ["E", "S", "s", "e"] {
            let cases = [("e1", "G^"), ("e8", "g^"), ("a4", token)];
            let g = board(&cases);
            assert!(!is_dead_position(XIONGQI, &g, &[], &[]), "{token}");
        }
    }

    // ── Mixed pairings: interactions-*.md, unchanged ──────────────────────────

    #[test]
    fn mixed_king_versus_general_draws_by_deliberate_ruling() {
        // Not strictly a dead position — a cornered General is matable in
        // principle — but no side can force a win (deciders' ruling,
        // 2026-07-10; interactions-*-xiongqi.md §End of Game).
        let b = board(&[("e1", "K^"), ("e8", "g^")]);
        assert!(is_dead_position(CHESS_V_XIONGQI, &b, &[], &[]));
        let g = board(&[("e1", "K^"), ("e8", "g^")]);
        assert!(is_dead_position(OGI_V_XIONGQI, &g, &[], &[]));
    }

    #[test]
    fn mixed_lone_royals_with_inert_hands_draw() {
        // Chess vs ōgi: the chess side's captures are inert (opponent's case).
        let b = board(&[("e1", "K^"), ("e8", "k^")]);
        assert!(is_dead_position(CHESS_V_OGI, &b, &[piece("p")], &[]));
    }

    #[test]
    fn mixed_droppable_hand_revives_the_game() {
        // Ōgi vs xiongqi: a droppable Fu in the ōgi hand keeps the game alive.
        let b = board(&[("e1", "K^"), ("e8", "g^")]);
        assert!(!is_dead_position(OGI_V_XIONGQI, &b, &[piece("F")], &[]));
    }

    #[test]
    fn mixed_board_piece_is_sufficient() {
        let b = board(&[("e1", "K^"), ("e8", "g^"), ("a1", "R")]);
        assert!(!is_dead_position(CHESS_V_XIONGQI, &b, &[], &[]));
    }

    #[test]
    fn mixed_pairs_do_not_inherit_the_chess_material_sets() {
        // K + B versus G is NOT a detected dead position in a mixed session:
        // the chess-only material sets apply to pure-chess sessions alone.
        let b = board(&[("e1", "K^"), ("e8", "g^"), ("h1", "B")]);
        assert!(!is_dead_position(CHESS_V_XIONGQI, &b, &[], &[]));
    }
}

/// Property-based tests: the invariants that the enumerated cases above cannot
/// exhaust. Boards are arbitrary piece placements — the detection is a pure
/// material census and never requires the position to be reachable or legal.
#[cfg(test)]
mod properties {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects
    )]

    use super::is_dead_position;
    use crate::domain::piece::Piece;
    use crate::domain::side::Side;
    use crate::domain::square::Square;
    use crate::domain::variant::{Variant, VariantAssignment};
    use proptest::collection::{hash_set, vec};
    use proptest::prelude::*;
    use sashite_epin::Identifier as Epin;
    use std::collections::HashMap;

    const CHESS: VariantAssignment = VariantAssignment {
        first: Variant::Chess,
        second: Variant::Chess,
    };
    const OGI: VariantAssignment = VariantAssignment {
        first: Variant::Ogi,
        second: Variant::Ogi,
    };

    fn piece(token: &str) -> Piece {
        Piece::new(Epin::parse(token).expect("valid EPIN"))
    }

    /// Flips a token's side (its case), preserving any `^` royal marker.
    fn flip(token: &str) -> String {
        token
            .chars()
            .map(|c| {
                if c.is_ascii_uppercase() {
                    c.to_ascii_lowercase()
                } else if c.is_ascii_lowercase() {
                    c.to_ascii_uppercase()
                } else {
                    c
                }
            })
            .collect()
    }

    type Board = HashMap<Square, Piece>;

    fn at(board: &Board) -> impl Fn(Square) -> Option<Piece> + '_ {
        move |s| board.get(&s).copied()
    }

    /// Places the two Kings on `sqs[0]`/`sqs[1]` and `tokens` on the squares
    /// that follow. `sqs` must therefore hold at least `tokens.len() + 2`
    /// distinct squares.
    fn board_with(tokens: &[&str], sqs: &[Square]) -> Board {
        let mut board: Board = HashMap::new();
        board.insert(sqs[0], piece("K^"));
        board.insert(sqs[1], piece("k^"));
        for (i, token) in tokens.iter().enumerate() {
            board.insert(sqs[i + 2], piece(token));
        }
        board
    }

    /// Twelve distinct squares — enough for every property below (at most ten
    /// pieces are ever placed).
    fn twelve_squares() -> impl Strategy<Value = Vec<Square>> {
        hash_set((0u8..8, 0u8..8), 12).prop_map(|s| {
            s.into_iter()
                .map(|(f, r)| Square::new(f, r).expect("on board"))
                .collect()
        })
    }

    /// A chess-material token: any piece a pure-chess board can carry.
    fn arb_chess_token() -> impl Strategy<Value = &'static str> {
        prop_oneof![
            Just("Q"),
            Just("R"),
            Just("B"),
            Just("N"),
            Just("P"),
            Just("q"),
            Just("r"),
            Just("b"),
            Just("n"),
            Just("p"),
        ]
    }

    /// An arbitrary hand: up to three pieces of either side.
    fn arb_hand() -> impl Strategy<Value = Vec<Piece>> {
        vec(arb_chess_token().prop_map(piece), 0..4)
    }

    proptest! {
        /// Pure chess: the hands are inert and NEVER influence the verdict,
        /// whatever material the board carries.
        #[test]
        fn chess_hands_never_influence(
            tokens in vec(arb_chess_token(), 0..5),
            sqs in twelve_squares(),
            first_hand in arb_hand(),
            second_hand in arb_hand(),
        ) {
            let board = board_with(&tokens, &sqs);
            prop_assert_eq!(
                is_dead_position(CHESS, at(&board), &first_hand, &second_hand),
                is_dead_position(CHESS, at(&board), &[], &[]),
            );
        }

        /// Pure chess: flipping every piece's side leaves the verdict
        /// unchanged — the census counts material globally, never per side.
        #[test]
        fn chess_side_swap_is_invariant(
            tokens in vec(arb_chess_token(), 0..5),
            sqs in twelve_squares(),
        ) {
            let flipped_owned: Vec<String> = tokens.iter().map(|t| flip(t)).collect();
            let flipped: Vec<&str> = flipped_owned.iter().map(String::as_str).collect();
            let original = board_with(&tokens, &sqs);
            let mirrored = board_with(&flipped, &sqs);
            prop_assert_eq!(
                is_dead_position(CHESS, at(&original), &[], &[]),
                is_dead_position(CHESS, at(&mirrored), &[], &[]),
            );
        }

        /// Pure chess: a Queen, a Rook, or a Pawn anywhere on the board — of
        /// either side, alongside any other material — is never a dead position.
        #[test]
        fn chess_heavy_piece_or_pawn_is_never_dead(
            mut tokens in vec(arb_chess_token(), 0..4),
            heavy in prop_oneof![
                Just("Q"), Just("R"), Just("P"),
                Just("q"), Just("r"), Just("p"),
            ],
            sqs in twelve_squares(),
        ) {
            tokens.push(heavy);
            let board = board_with(&tokens, &sqs);
            prop_assert!(!is_dead_position(CHESS, at(&board), &[], &[]));
        }

        /// Pure chess: Kings + any number of Bishops all standing on squares
        /// of the SAME colour is dead — and adding a single opposite-coloured
        /// Bishop revives the game.
        #[test]
        fn chess_same_colour_bishops_are_dead(
            parity in 0u8..2,
            count in 1usize..6,
            mixed_sides in proptest::bool::ANY,
            sqs in twelve_squares(),
        ) {
            let mut board: Board = HashMap::new();
            board.insert(sqs[0], piece("K^"));
            board.insert(sqs[1], piece("k^"));
            let mut placed = 0usize;
            let mut opposite: Option<Square> = None;
            for &sq in &sqs[2..] {
                let colour = (sq.file() ^ sq.rank()) & 1;
                if colour == parity && placed < count {
                    let token = if mixed_sides && placed % 2 == 0 { "b" } else { "B" };
                    board.insert(sq, piece(token));
                    placed += 1;
                } else if colour != parity && opposite.is_none() {
                    opposite = Some(sq);
                }
            }
            // Ten free squares always hold at least one of each colour class.
            prop_assume!(placed > 0);
            prop_assert!(is_dead_position(CHESS, at(&board), &[], &[]));
            if let Some(sq) = opposite {
                board.insert(sq, piece("b"));
                prop_assert!(!is_dead_position(CHESS, at(&board), &[], &[]));
            }
        }

        /// Pure chess: two or more Knights — however split between the sides —
        /// or a Knight alongside a Bishop, is never dead (the criterion is
        /// possibility, not forceability).
        #[test]
        fn chess_knight_pairs_and_mixes_are_never_dead(
            knights in 2usize..5,
            with_bishop in proptest::bool::ANY,
            sqs in twelve_squares(),
        ) {
            let mut tokens: Vec<&str> = Vec::new();
            for i in 0..knights {
                tokens.push(if i % 2 == 0 { "N" } else { "n" });
            }
            if with_bishop {
                tokens.truncate(1); // one Knight + one Bishop instead
                tokens.push("B");
            }
            let board = board_with(&tokens, &sqs);
            prop_assert!(!is_dead_position(CHESS, at(&board), &[], &[]));
        }

        /// Pure ōgi NEVER declares a dead position, whatever the board and the
        /// hands — the detection set is empty by specification.
        #[test]
        fn ogi_never_declares(
            tokens in vec(arb_chess_token(), 0..5),
            sqs in twelve_squares(),
            first_hand in arb_hand(),
            second_hand in arb_hand(),
        ) {
            let board = board_with(&tokens, &sqs);
            prop_assert!(!is_dead_position(OGI, at(&board), &first_hand, &second_hand));
        }

        /// Mixed pairings, only the two royals on the board: the verdict is
        /// EXACTLY the negation of the droppable-hand criterion — inert pieces
        /// never matter, one own-case piece always revives.
        #[test]
        fn mixed_verdict_is_exactly_the_droppable_criterion(
            first_hand in arb_hand(),
            second_hand in arb_hand(),
            pairing in prop_oneof![
                Just(VariantAssignment { first: Variant::Chess, second: Variant::Ogi }),
                Just(VariantAssignment { first: Variant::Ogi, second: Variant::Xiongqi }),
                Just(VariantAssignment { first: Variant::Chess, second: Variant::Xiongqi }),
            ],
            sqs in twelve_squares(),
        ) {
            let mut board: Board = HashMap::new();
            board.insert(sqs[0], piece("K^"));
            board.insert(sqs[1], piece("g^"));
            let droppable = first_hand.iter().any(|p| p.belongs_to(Side::First))
                || second_hand.iter().any(|p| p.belongs_to(Side::Second));
            prop_assert_eq!(
                is_dead_position(pairing, at(&board), &first_hand, &second_hand),
                !droppable,
            );
        }
    }
}
