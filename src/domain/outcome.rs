//! `Verdict`, `IllegalReason`, and the `Outcome` structure of an applied ply.
//!
//! An [`Outcome`] describes a ply the kernel has **applied** (next canonical
//! FEEN, clocks, verdict). An illegal ply never produces an `Outcome`: the
//! kernel rejects it with the precise [`IllegalReason`], handing the state
//! back — the "reject, never terminate" contract of statuses-sanki (there is
//! no `illegalmove` status). See `kernel::step::StepResult`.

use crate::domain::side::Side;
use crate::domain::status::{Outcome3, ResultKind, Status};
use crate::domain::time_control::Clocks;

/// A session's verdict after applying a move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Verdict {
    /// The session continues.
    Ongoing,
    /// The session has ended, with its status and point split.
    Terminated {
        /// Termination cause.
        status: Status,
        /// Outcome (point split).
        result: Outcome3,
    },
}

impl Verdict {
    /// Draw verdict for a draw status (outcome `50/50`).
    #[inline]
    #[must_use]
    pub const fn drawn(status: Status) -> Self {
        Self::Terminated {
            status,
            result: Outcome3::Draw,
        }
    }

    /// Decisive verdict at the expense of `loser`, for a decisive status.
    ///
    /// The outcome is derived from `loser` ([`Outcome3::loss_for`]): the split
    /// therefore cannot be inconsistent with the designated loser.
    #[inline]
    #[must_use]
    pub const fn decisive(status: Status, loser: Side) -> Self {
        Self::Terminated {
            status,
            result: Outcome3::loss_for(loser),
        }
    }

    /// True if the session has ended.
    #[inline]
    #[must_use]
    pub const fn is_terminated(self) -> bool {
        matches!(self, Self::Terminated { .. })
    }

    /// Status ↔ outcome consistency: a draw status yields a draw outcome, a
    /// decisive status a decisive outcome. An invariant the [`Self::drawn`] /
    /// [`Self::decisive`] constructors guarantee by construction.
    #[inline]
    #[must_use]
    pub const fn is_consistent(self) -> bool {
        match self {
            Self::Ongoing => true,
            Self::Terminated { status, result } => match status.result_kind() {
                ResultKind::Draw => matches!(result, Outcome3::Draw),
                ResultKind::Decisive => !matches!(result, Outcome3::Draw),
            },
        }
    }
}

/// The precise cause of a move judged illegal.
///
/// Diagnostic accompanying a kernel rejection (`StepResult::Illegal`) or a
/// façade `validate`/`apply` error. The taxonomy is deliberately still coarse:
/// it may gain further variants (e.g. distinguishing nifu / last rank) — the
/// exhaustive `match`es will then point out each site to update. Uchifuzume,
/// long folded into [`IllegalReason::IllegalDrop`], has its dedicated variant
/// since 0.4.0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IllegalReason {
    /// Malformed Ply content (invalid square / actor, non-conforming triple).
    Malformed,
    /// No piece of the side to move on the source square.
    NoMoverPieceAtSource,
    /// Destination unreachable by the piece given its movement.
    IllegalDestination,
    /// Sliding through an occupied square.
    BlockedPath,
    /// Destination occupied by a friendly piece.
    FriendlyDestination,
    /// The move leaves the player's royal in check (FIDE self-check filter).
    LeavesRoyalInCheck,
    /// Castling whose conditions are not met.
    IllegalCastling,
    /// Illegal en-passant capture.
    IllegalEnPassant,
    /// Double step with an ineligible foot-soldier.
    IllegalDoubleStep,
    /// Illegal drop (dead piece, last rank, nifu, occupied square…).
    IllegalDrop,
    /// Mating Fu drop (uchifuzume, ōgi): a Fu drop may not deliver checkmate.
    Uchifuzume,
    /// Illegal promotion, or a mandatory promotion omitted.
    IllegalPromotion,
}

/// The outcome of an **applied** ply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// Next canonical position (FEEN).
    pub position: String,
    /// Updated clocks.
    pub clocks: Clocks,
    /// Verdict (ongoing or terminated).
    pub verdict: Verdict,
}

impl Outcome {
    /// Outcome of a legal, non-terminal move: the game continues.
    #[inline]
    #[must_use]
    pub fn ongoing(position: String, clocks: Clocks) -> Self {
        Self {
            position,
            clocks,
            verdict: Verdict::Ongoing,
        }
    }

    /// General outcome: verdict supplied by the caller (the kernel).
    #[inline]
    #[must_use]
    pub fn new(position: String, clocks: Clocks, verdict: Verdict) -> Self {
        Self {
            position,
            clocks,
            verdict,
        }
    }
}

impl core::fmt::Display for IllegalReason {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let msg = match self {
            Self::Malformed => "malformed Ply content",
            Self::NoMoverPieceAtSource => "no piece of the side to move on the source",
            Self::IllegalDestination => "unreachable destination",
            Self::BlockedPath => "sliding through an occupied square",
            Self::FriendlyDestination => "destination occupied by a friendly piece",
            Self::LeavesRoyalInCheck => "the move leaves the royal in check",
            Self::IllegalCastling => "illegal castling",
            Self::IllegalEnPassant => "illegal en-passant capture",
            Self::IllegalDoubleStep => "illegal double step",
            Self::IllegalDrop => "illegal drop",
            Self::Uchifuzume => "mating Fu drop (uchifuzume)",
            Self::IllegalPromotion => "illegal promotion",
        };
        f.write_str(msg)
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

    use super::{Clocks, Outcome3, Side, Status};
    use super::{Outcome, Verdict};
    use crate::domain::time::Duration;
    use crate::domain::time_control::{Period, TimeControl};

    /// A valid clock pair for tests (symmetric time control of a single 60 s
    /// period, no increment).
    fn sample_clocks() -> Clocks {
        let period = Period::new(Duration::from_secs(60), None, None).expect("valid period");
        let tc = TimeControl::new(period, Vec::new());
        Clocks::start(&tc)
    }

    #[test]
    fn drawn_is_consistent() {
        let v = Verdict::drawn(Status::Stalemate);
        assert_eq!(
            v,
            Verdict::Terminated {
                status: Status::Stalemate,
                result: Outcome3::Draw,
            },
        );
        assert!(v.is_terminated());
        assert!(v.is_consistent());
    }

    #[test]
    fn decisive_derives_outcome_from_loser() {
        // Loser = first player -> second player wins.
        let v = Verdict::decisive(Status::Checkmate, Side::First);
        assert_eq!(
            v,
            Verdict::Terminated {
                status: Status::Checkmate,
                result: Outcome3::SecondWins,
            },
        );
        assert!(v.is_consistent());

        let v2 = Verdict::decisive(Status::Checkmate, Side::Second);
        assert_eq!(
            v2,
            Verdict::Terminated {
                status: Status::Checkmate,
                result: Outcome3::FirstWins,
            },
        );
    }

    #[test]
    fn ongoing_is_consistent() {
        assert!(Verdict::Ongoing.is_consistent());
        assert!(!Verdict::Ongoing.is_terminated());
    }

    #[test]
    fn is_consistent_detects_inconsistencies() {
        // Draw status with a decisive outcome: inconsistent.
        let bad = Verdict::Terminated {
            status: Status::Stalemate,
            result: Outcome3::FirstWins,
        };
        assert!(!bad.is_consistent());
        // Decisive status with a draw outcome: inconsistent.
        let bad2 = Verdict::Terminated {
            status: Status::Checkmate,
            result: Outcome3::Draw,
        };
        assert!(!bad2.is_consistent());
    }

    #[test]
    fn outcome_ongoing() {
        let o = Outcome::ongoing("8/8/8/8/8/8/8/8 / W/w".to_owned(), sample_clocks());
        assert_eq!(o.verdict, Verdict::Ongoing);
        assert_eq!(o.position, "8/8/8/8/8/8/8/8 / W/w");
    }

    #[test]
    fn outcome_terminated() {
        let verdict = Verdict::decisive(Status::Timeout, Side::First);
        let o = Outcome::new("pos".to_owned(), sample_clocks(), verdict);
        assert!(o.verdict.is_terminated());
        assert!(o.verdict.is_consistent());
    }
}
