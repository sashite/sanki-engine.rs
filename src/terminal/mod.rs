//! Terminations derived from the position (and history).
//!
//! The submodules **compute** the individual conditions:
//! - [`legal_set`]: existence of a legal (and, in the end, pseudo-legal) move;
//! - [`dead_position`]: the dead-position draw (status `insufficient`);
//! - [`repetition`]: threefold repetition;
//! - [`move_limit`]: the 100-half-move rule.
//!
//! [`classify`] **orders** these conditions into a single [`Verdict`] following
//! the normative order. It is a **pure** ordering policy: it consumes only
//! already-computed facts (by the kernel, via `check` and the submodules above),
//! which makes it trivial to test and independent of the position's
//! representation.

pub mod dead_position;
pub mod legal_set;
pub mod move_limit;
pub mod repetition;
pub mod uchifuzume;

use crate::domain::outcome::Verdict;
use crate::domain::side::Side;
use crate::domain::status::Status;

/// Terminal facts of a position, from the **side to move**'s point of view.
///
/// All evaluated for `side_to_move`. Expected invariant: `has_legal_move` implies
/// `has_pseudo_legal_move` (the legal set is included in the pseudo-legal one) —
/// [`classify`] nonetheless stays correct as long as it is respected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalConditions {
    /// Side to move (the loser on checkmate).
    pub side_to_move: Side,
    /// The side to move's royal is in check.
    pub in_check: bool,
    /// The side to move can generate at least one **pseudo-legal** move.
    pub has_pseudo_legal_move: bool,
    /// The side to move has at least one **legal** move.
    pub has_legal_move: bool,
    /// Dead position (insufficient material): no checkmate is possible by any
    /// series of legal moves — the variant-specific material-only detection.
    pub insufficient: bool,
    /// The current position occurs for the third time.
    pub threefold_repetition: bool,
    /// The 100-half-move counter is reached.
    pub move_limit_reached: bool,
}

/// Reduces the terminal conditions into a [`Verdict`], following the normative
/// order.
///
/// 1. **no legal move**:
///    - no *pseudo-legal* move → `nomove` (draw);
///    - else, in check → `checkmate` (the side to move **loses**);
///    - else → `stalemate` (draw);
/// 2. **the game continues** — background draws, in order:
///    `insufficient` → `repetition` → `movelimit`;
/// 3. else → `Ongoing`.
///
/// We branch first on `has_legal_move`: a **delivered mate takes precedence over
/// any background draw** — including `insufficient` arising on the very same
/// ply. The simultaneity is real in exactly one configuration: the deliberate
/// cross-variant King-vs-General `insufficient` exception, whose cornered
/// General *is* matable in principle (interactions-*-xiongqi.md §End of Game).
/// Per the deciders' ruling of 2026-07-19, a mate already on the board wins:
/// the "declared drawn outright" reading of K-vs-G spares *ongoing* positions a
/// forced shuffle, it does not annul a delivered mate. (In the no-legal-move
/// branch, a stalemate coinciding with a dead position resolves to the
/// `stalemate` token — both are `50/50`, only the label differs.)
#[must_use]
pub const fn classify(conditions: TerminalConditions) -> Verdict {
    if conditions.has_legal_move {
        // The game may continue: only the "background" draws apply.
        if conditions.insufficient {
            return Verdict::drawn(Status::Insufficient);
        }
        if conditions.threefold_repetition {
            return Verdict::drawn(Status::Repetition);
        }
        if conditions.move_limit_reached {
            return Verdict::drawn(Status::MoveLimit);
        }
        return Verdict::Ongoing;
    }

    // No legal move: nomove (priority), then mate or stalemate.
    if !conditions.has_pseudo_legal_move {
        return Verdict::drawn(Status::NoMove);
    }
    if conditions.in_check {
        return Verdict::decisive(Status::Checkmate, conditions.side_to_move);
    }
    Verdict::drawn(Status::Stalemate)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::{classify, TerminalConditions};
    use crate::domain::outcome::Verdict;
    use crate::domain::side::Side;
    use crate::domain::status::Status;

    /// "Game ongoing" conditions: a legal move exists, no draw.
    const ONGOING: TerminalConditions = TerminalConditions {
        side_to_move: Side::First,
        in_check: false,
        has_pseudo_legal_move: true,
        has_legal_move: true,
        insufficient: false,
        threefold_repetition: false,
        move_limit_reached: false,
    };

    #[test]
    fn game_ongoing() {
        assert_eq!(classify(ONGOING), Verdict::Ongoing);
    }

    #[test]
    fn checkmate_makes_the_side_to_move_lose() {
        let c = TerminalConditions {
            in_check: true,
            has_legal_move: false,
            ..ONGOING
        };
        assert_eq!(
            classify(c),
            Verdict::decisive(Status::Checkmate, Side::First)
        );
    }

    #[test]
    fn stalemate_is_a_draw() {
        let c = TerminalConditions {
            in_check: false,
            has_legal_move: false,
            ..ONGOING
        };
        assert_eq!(classify(c), Verdict::drawn(Status::Stalemate));
    }

    #[test]
    fn nomove_takes_priority_over_mate_and_stalemate() {
        // No pseudo-legal move: nomove, even in check.
        let c = TerminalConditions {
            in_check: true,
            has_pseudo_legal_move: false,
            has_legal_move: false,
            ..ONGOING
        };
        assert_eq!(classify(c), Verdict::drawn(Status::NoMove));
    }

    #[test]
    fn background_draws() {
        let insuf = TerminalConditions {
            insufficient: true,
            ..ONGOING
        };
        assert_eq!(classify(insuf), Verdict::drawn(Status::Insufficient));

        let rep = TerminalConditions {
            threefold_repetition: true,
            ..ONGOING
        };
        assert_eq!(classify(rep), Verdict::drawn(Status::Repetition));

        let lim = TerminalConditions {
            move_limit_reached: true,
            ..ONGOING
        };
        assert_eq!(classify(lim), Verdict::drawn(Status::MoveLimit));
    }

    #[test]
    fn checkmate_outranks_insufficient_on_the_same_ply() {
        // The K-vs-G simultaneity (deciders' ruling, 2026-07-19): a ply that
        // both mates and leaves a detected dead configuration is a checkmate.
        let c = TerminalConditions {
            in_check: true,
            has_legal_move: false,
            insufficient: true,
            ..ONGOING
        };
        assert_eq!(
            classify(c),
            Verdict::decisive(Status::Checkmate, Side::First)
        );
    }

    #[test]
    fn mate_outranks_repetition() {
        // In check with no legal move while a repetition would be claimable:
        // checkmate (decisive) wins.
        let c = TerminalConditions {
            in_check: true,
            has_legal_move: false,
            threefold_repetition: true,
            move_limit_reached: true,
            ..ONGOING
        };
        assert_eq!(
            classify(c),
            Verdict::decisive(Status::Checkmate, Side::First)
        );
    }

    #[test]
    fn insufficient_outranks_repetition_and_movelimit() {
        let c = TerminalConditions {
            insufficient: true,
            threefold_repetition: true,
            move_limit_reached: true,
            ..ONGOING
        };
        assert_eq!(classify(c), Verdict::drawn(Status::Insufficient));
    }

    #[test]
    fn checkmate_of_the_second_player() {
        let c = TerminalConditions {
            side_to_move: Side::Second,
            in_check: true,
            has_legal_move: false,
            ..ONGOING
        };
        assert_eq!(
            classify(c),
            Verdict::decisive(Status::Checkmate, Side::Second)
        );
    }
}
