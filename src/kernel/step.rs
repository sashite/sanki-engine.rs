//! `step` — the L1 kernel: one pure per-ply transition.
//!
//! `step` assembles the layers built below it into a single uniform result:
//!
//! ```text
//! resolve (legality) → apply → canonicalize → tick (clock) → terminal
//! ```
//!
//! It always yields a [`StepResult`], never an `Err` and never a panic:
//!
//! - an **illegal** ply (full rule system, uchifuzume included) is **rejected**,
//!   not sanctioned: [`StepResult::Illegal`] hands the untouched
//!   [`SessionState`] back with the precise [`IllegalReason`] — the player
//!   keeps the turn, the clocks do not move. There is **no** `illegalmove`
//!   termination (statuses-sanki §Verdict resolution: an illegal Ply is
//!   skipped, never a loss);
//! - otherwise the ply is applied and canonicalized, then the mover's clock is
//!   ticked; a **flag** terminates against the mover (`timeout`);
//! - finally the resulting position is classified for terminal conditions
//!   (checkmate, stalemate, nomove, and the background draws) —
//!   [`StepResult::Advanced`], with the successor state when the session
//!   continues.
//!
//! Should `apply` or `canonicalize` fail on an already-legal effect — a broken
//! internal invariant, unreachable on well-formed input — the ply is treated
//! defensively as a rejection ([`IllegalReason::Malformed`]) rather than by
//! panicking.

use crate::apply::{apply, Effect};
use crate::canonicalize::canonicalize;
use crate::clock::tick;
use crate::domain::half_move::Move;
use crate::domain::outcome::{IllegalReason, Outcome, Verdict};
use crate::domain::piece::Piece;
use crate::domain::side::Side;
use crate::domain::square::Square;
use crate::domain::status::Status;
use crate::domain::time::{Duration, Timestamp};
use crate::domain::time_control::Clocks;
use crate::kernel::state::SessionState;
use crate::legality::check::in_check;
use crate::legality::resolve::resolve;
use crate::position::Position;
use crate::terminal::dead_position::is_dead_position;
use crate::terminal::legal_set::{has_full_legal_move, has_pseudo_legal_move};
use crate::terminal::move_limit::clock_resets;
use crate::terminal::uchifuzume::is_uchifuzume_drop;
use crate::terminal::{classify, TerminalConditions};

/// Result of a kernel step.
#[derive(Debug, Clone)]
pub enum StepResult {
    /// The ply is illegal under the full rule system: the session is
    /// **unchanged** and the player keeps the turn — an illegal ply is never a
    /// termination. The state is handed back for the caller to thread.
    Illegal {
        /// The untouched session state.
        state: SessionState,
        /// The precise cause of the rejection.
        reason: IllegalReason,
    },
    /// The ply was applied: the per-ply [`Outcome`] (next canonical FEEN,
    /// clocks, verdict), plus the successor state when the session continues.
    Advanced {
        /// The per-ply outcome.
        outcome: Outcome,
        /// The next state — `Some` iff the verdict is `Ongoing`, `None` once
        /// the session has terminated.
        next: Option<SessionState>,
    },
}

/// Advances the session by one ply, consuming `state` and producing the uniform
/// [`StepResult`].
///
/// `mv` is the parsed half-move (the boundary handles parsing); `attestation_at`
/// is the ply's canonical timestamper `created_at`, against which the elapsed
/// time is measured from the state's anchor.
#[must_use]
pub fn step(state: SessionState, mv: &Move, attestation_at: Timestamp) -> StepResult {
    let mover = state.position().active_side();

    // 1. Legality: a rejection hands the state back, the player keeps the turn.
    let effect = match resolve(state.position(), mv) {
        Ok(effect) => effect,
        Err(reason) => return StepResult::Illegal { state, reason },
    };

    // 1b. Uchifuzume (ōgi): a Fu drop may not deliver checkmate. Resolved here,
    // above the `legality` layer, because the rule composes the terminal layer
    // (the opponent's legal-move search) over it — the same composition the
    // `engine` façade performs in `resolve_full`. `is_uchifuzume_drop` is inert
    // for any non-Fu drop, so the guard is a no-op outside ōgi Fu drops.
    if let Effect::Drop { piece, to } = effect {
        if is_uchifuzume_drop(state.position(), piece, to) {
            return StepResult::Illegal {
                state,
                reason: IllegalReason::Uchifuzume,
            };
        }
    }

    // 2. Apply and canonicalize (defensive on the unreachable invariant break:
    // a rejection, never a termination and never a panic).
    let Ok(applied) = apply(state.position(), effect) else {
        return StepResult::Illegal {
            state,
            reason: IllegalReason::Malformed,
        };
    };
    let Ok(canonical) = canonicalize(&applied, &effect) else {
        return StepResult::Illegal {
            state,
            reason: IllegalReason::Malformed,
        };
    };

    // 3. Tick the mover's clock from the attestation-anchored elapsed time.
    // `duration_since` is `None` in two cases that must NOT be conflated: a ply
    // timed before the anchor — the premove clamp, `elapsed = max(0, t − anchor)`
    // (time-accounting §Elapsed time) — and a forward difference overflowing the
    // representation (an anchor near `i64::MIN`, unreachable with genuine Nostr
    // timings). Charging zero for the latter would let an astronomically late
    // ply pass free; it saturates instead, flagging under any finite control.
    let anchor = state.last_attestation();
    let elapsed = match attestation_at.duration_since(anchor) {
        Some(elapsed) => elapsed,
        None if attestation_at < anchor => Duration::ZERO,
        None => Duration::from_secs(u64::MAX),
    };
    let clocks = match tick(state.time_control(), state.clocks().get(mover), elapsed).clock() {
        Some(clock) => state.clocks().with(mover, clock),
        None => {
            // The mover flagged: a loss on time. The clocks reported are those
            // entering the ply (the mover's budget is spent regardless).
            return terminated(
                canonical.to_feen(),
                state.clocks(),
                Verdict::decisive(Status::Timeout, mover),
            );
        }
    };

    // 4. Thread the next state before reading history-dependent terminal facts.
    let feen = canonical.to_feen();
    let irreversible = resets_move_limit(state.position(), &effect);
    let next_state = state.advance(canonical, clocks, attestation_at, irreversible);

    // 5. Terminal classification on the resulting position.
    let verdict = classify_terminal(&next_state);
    if verdict.is_terminated() {
        terminated(feen, clocks, verdict)
    } else {
        StepResult::Advanced {
            outcome: Outcome::ongoing(feen, clocks),
            next: Some(next_state),
        }
    }
}

/// Builds a terminated result (no successor state).
fn terminated(position: String, clocks: Clocks, verdict: Verdict) -> StepResult {
    StepResult::Advanced {
        outcome: Outcome::new(position, clocks, verdict),
        next: None,
    }
}

/// Whether the ply resets the 50-move half-move clock, per
/// [`crate::terminal::move_limit::clock_resets`] — the single source of the
/// rule: a capture, or a board move of an unpromoted pawn-class piece (read on
/// the source position, so a promotion — whose `placed` is the promoted piece —
/// still counts). Neither a drop nor castling resets the clock (the king is
/// not a pawn-class piece, and a drop is not a board move).
#[inline]
fn resets_move_limit(position: &Position, effect: &Effect) -> bool {
    match effect {
        Effect::Board { from, captured, .. } => {
            clock_resets(position.piece_at(*from), captured.is_some())
        }
        Effect::Castle(_) | Effect::Drop { .. } => clock_resets(None, false),
    }
}

/// Classifies the resulting position for the side to move into a [`Verdict`].
fn classify_terminal(state: &SessionState) -> Verdict {
    let position = state.position();
    let side = position.active_side();
    let variants = position.variants();
    let opponent_variant = position.variant_of(side.flip());
    let piece_at = |square: Square| position.piece_at(square);

    let first_hand: Vec<Piece> = position.hand(Side::First).map(|(piece, _)| piece).collect();
    let second_hand: Vec<Piece> = position
        .hand(Side::Second)
        .map(|(piece, _)| piece)
        .collect();
    // Both hands in one list: the predicates droppable-filter by side, and the
    // full reading's uchifuzume mate test needs the opponent's hand too.
    let hands: Vec<Piece> = first_hand
        .iter()
        .chain(second_hand.iter())
        .copied()
        .collect();

    let legal = has_full_legal_move(side, variants, piece_at, &hands);
    classify(TerminalConditions {
        side_to_move: side,
        in_check: in_check(side, opponent_variant, piece_at),
        // The pseudo-legal set is only consulted when no legal move exists.
        has_pseudo_legal_move: legal || has_pseudo_legal_move(side, variants, piece_at, &hands),
        has_legal_move: legal,
        insufficient: is_dead_position(variants, piece_at, &first_hand, &second_hand),
        threefold_repetition: state.threefold_repetition(),
        move_limit_reached: state.move_limit_reached(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::{step, StepResult};
    use crate::domain::half_move::Move;
    use crate::domain::outcome::{IllegalReason, Outcome, Verdict};
    use crate::domain::side::Side;
    use crate::domain::status::Status;
    use crate::domain::time::{Duration, Timestamp};
    use crate::domain::time_control::{Period, TimeControl};
    use crate::kernel::state::SessionState;
    use crate::position::Position;

    fn time_control(secs: u64) -> TimeControl {
        let period = Period::new(Duration::from_secs(secs), None, None).expect("valid period");
        TimeControl::new(period, Vec::new())
    }

    fn state(feen: &str, secs: u64) -> SessionState {
        let position = Position::parse(feen).expect("valid Sanki FEEN");
        SessionState::start(position, time_control(secs), Timestamp::from_unix(0))
    }

    fn mv(content: &str) -> Move {
        Move::parse(content).expect("valid ply content")
    }

    /// Unwraps an applied ply (panics on a rejection — test helper).
    fn advanced(result: StepResult) -> (Outcome, Option<SessionState>) {
        match result {
            StepResult::Advanced { outcome, next } => (outcome, next),
            StepResult::Illegal { reason, .. } => {
                panic!("expected an applied ply, got a rejection: {reason:?}")
            }
        }
    }

    #[test]
    fn legal_move_continues_and_increments_counter() {
        let result = step(
            state("4k^3/8/8/8/8/8/8/R3K^3 / W/w", 600),
            &mv("[\"a1\",\"a4\",null]"),
            Timestamp::from_unix(30),
        );
        let (outcome, next) = advanced(result);
        assert_eq!(outcome.verdict, Verdict::Ongoing);
        let next = next.expect("the game continues");
        assert_eq!(next.half_move(), 2);
        // Quiet move (neither capture nor foot-soldier): the counter increments.
        assert_eq!(next.halfmove_clock(), 1);
    }

    #[test]
    fn illegal_move_is_rejected_with_the_state_handed_back() {
        // No piece of the side to move on the source square: the ply is
        // rejected, never a termination — the player keeps the turn and the
        // untouched state comes back (clocks unmoved, same half-move).
        let result = step(
            state("4k^3/8/8/8/8/8/8/4K^3 / W/w", 600),
            &mv("[\"a1\",\"a4\",null]"),
            Timestamp::from_unix(30),
        );
        match result {
            StepResult::Illegal { state, reason } => {
                assert_eq!(reason, IllegalReason::NoMoverPieceAtSource);
                assert_eq!(state.position().to_feen(), "4k^3/8/8/8/8/8/8/4K^3 / W/w");
                assert_eq!(state.half_move(), 1);
            }
            StepResult::Advanced { .. } => panic!("an illegal ply must be rejected"),
        }
    }

    #[test]
    fn checkmate_terminates_game() {
        // Back-rank mate: Ra1→a8 mates the black King h8 walled in by its pawns.
        let result = step(
            state("7k^/6pp/8/8/8/8/8/R3K^3 / W/w", 600),
            &mv("[\"a1\",\"a8\",null]"),
            Timestamp::from_unix(30),
        );
        let (outcome, next) = advanced(result);
        assert_eq!(
            outcome.verdict,
            Verdict::decisive(Status::Checkmate, Side::Second)
        );
        assert!(next.is_none());
    }

    #[test]
    fn timeout_terminates_game() {
        // 5 s bank, but 100 s elapsed: the side to move overruns.
        let result = step(
            state("4k^3/8/8/8/8/8/8/R3K^3 / W/w", 5),
            &mv("[\"a1\",\"a4\",null]"),
            Timestamp::from_unix(100),
        );
        let (outcome, next) = advanced(result);
        assert_eq!(
            outcome.verdict,
            Verdict::decisive(Status::Timeout, Side::First)
        );
        assert!(next.is_none());
    }

    #[test]
    fn capture_resets_counter() {
        // Ra1xa5 captures the black Rook without giving check: game ongoing,
        // 50-move counter reset.
        let result = step(
            state("4k^3/8/8/r7/8/8/8/R3K^3 / W/w", 600),
            &mv("[\"a1\",\"a5\",null]"),
            Timestamp::from_unix(30),
        );
        let (outcome, next) = advanced(result);
        assert_eq!(outcome.verdict, Verdict::Ongoing);
        let next = next.expect("the game continues");
        assert_eq!(next.halfmove_clock(), 0);
    }

    #[test]
    fn capture_into_dead_position_terminates_with_insufficient() {
        // Kxd1 removes the last black piece: King + Bishop versus King is an
        // immediate dead-position draw, pinned here at the kernel level
        // (rules-of-chess §Dead-Position Detection).
        let result = step(
            state("4k^3/8/8/8/8/8/8/3rK^2B / W/w", 600),
            &mv("[\"e1\",\"d1\",null]"),
            Timestamp::from_unix(30),
        );
        let (outcome, next) = advanced(result);
        assert_eq!(outcome.verdict, Verdict::drawn(Status::Insufficient));
        assert!(next.is_none());
    }

    #[test]
    fn ogi_lone_kings_continue_without_dead_position_detection() {
        // Pure ōgi performs no dead-position detection (rules-of-ogi
        // §Dead-Position Detection): even the composed lone-Kings-empty-hands
        // configuration — unreachable from the standard start — plays on.
        let result = step(
            state("4k^3/8/8/8/8/8/8/4K^3 / J/j", 600),
            &mv("[\"e1\",\"d1\",null]"),
            Timestamp::from_unix(30),
        );
        let (outcome, next) = advanced(result);
        assert_eq!(outcome.verdict, Verdict::Ongoing);
        assert!(next.is_some());
    }

    #[test]
    fn premove_elapsed_clamps_to_zero() {
        // A ply timed BEFORE the anchor (a premove): elapsed clamps to zero
        // (time-accounting §Elapsed time) — no flag even on a tiny bank.
        let result = step(
            state("4k^3/8/8/8/8/8/8/R3K^3 / W/w", 5),
            &mv("[\"a1\",\"a4\",null]"),
            Timestamp::from_unix(-100),
        );
        let (outcome, next) = advanced(result);
        assert_eq!(outcome.verdict, Verdict::Ongoing);
        assert!(next.is_some());
    }

    #[test]
    fn forward_overflow_saturates_and_flags() {
        // An anchor near i64::MIN overflows the forward difference: the elapsed
        // saturates instead of clamping to zero, so the mover flags — an
        // astronomically late ply never passes free.
        let position = Position::parse("4k^3/8/8/8/8/8/8/R3K^3 / W/w").expect("valid Sanki FEEN");
        let s = SessionState::start(position, time_control(600), Timestamp::from_unix(i64::MIN));
        let result = step(s, &mv("[\"a1\",\"a4\",null]"), Timestamp::from_unix(0));
        let (outcome, next) = advanced(result);
        assert_eq!(
            outcome.verdict,
            Verdict::decisive(Status::Timeout, Side::First)
        );
        assert!(next.is_none());
    }

    #[test]
    fn mating_fu_drop_is_rejected_uchifuzume() {
        // Ōgi: black King walled in on h8, white Rook g1 and white Knight f6
        // (which defends h7); the first player holds a Fu in hand. Dropping it
        // on h7 would deliver checkmate -> rejected (uchifuzume); the session
        // continues, the player keeps the turn.
        let result = step(
            state("7k^/8/5N2/8/8/8/8/4K^1R1 F/ J/j", 600),
            &mv("[null,\"h7\",\"fu\"]"),
            Timestamp::from_unix(30),
        );
        match result {
            StepResult::Illegal { state, reason } => {
                assert_eq!(reason, IllegalReason::Uchifuzume);
                assert_eq!(
                    state.position().to_feen(),
                    "7k^/8/5N2/8/8/8/8/4K^1R1 F/ J/j"
                );
            }
            StepResult::Advanced { .. } => panic!("uchifuzume must be rejected"),
        }
    }

    #[test]
    fn checking_fu_drop_is_legal() {
        // Same position without the knight: the Fu on h7 is undefended, the King
        // captures it. Check without mate -> drop allowed, the game continues.
        let result = step(
            state("7k^/8/8/8/8/8/8/4K^1R1 F/ J/j", 600),
            &mv("[null,\"h7\",\"fu\"]"),
            Timestamp::from_unix(30),
        );
        let (outcome, next) = advanced(result);
        assert_eq!(outcome.verdict, Verdict::Ongoing);
        assert!(next.is_some());
    }
}
