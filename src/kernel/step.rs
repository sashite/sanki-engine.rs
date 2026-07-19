//! `step` — the L1 kernel: one pure per-ply transition.
//!
//! `step` assembles the layers built below it into a single uniform result:
//!
//! ```text
//! resolve (legality) → apply → canonicalize → tick (clock) → terminal
//! ```
//!
//! It **always** produces an [`Outcome`] (next canonical FEEN, clocks, verdict,
//! and illegality reason) — illegality is encoded in the verdict, never as an
//! `Err`. Alongside the outcome it returns the next [`SessionState`] when, and
//! only when, the session continues, so callers can thread plies.
//!
//! Ordering (per the kernel contract):
//! - an **illegal** ply terminates the session against the mover (`illegalmove`)
//!   with the position and clocks left unchanged;
//! - otherwise the ply is applied and canonicalized, then the mover's clock is
//!   ticked; a **flag** terminates against the mover (`timeout`);
//! - finally the resulting position is classified for terminal conditions
//!   (checkmate, stalemate, nomove, and the background draws).
//!
//! Should `apply` or `canonicalize` fail on an already-legal effect — a broken
//! internal invariant, unreachable on well-formed input — the ply is treated
//! defensively as malformed (`illegalmove`) rather than by panicking.

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
use crate::terminal::uchifuzume::is_uchifuzume_drop;
use crate::terminal::{classify, TerminalConditions};

/// Result of a kernel step: the uniform [`Outcome`], plus the next state when the
/// session continues.
#[derive(Debug, Clone)]
pub struct StepResult {
    /// The per-ply outcome (next canonical FEEN, clocks, verdict, reason).
    pub outcome: Outcome,
    /// The next state — `Some` iff the verdict is `Ongoing`, `None` once the
    /// session has terminated.
    pub next: Option<SessionState>,
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

    // 1. Legality.
    let effect = match resolve(state.position(), mv) {
        Ok(effect) => effect,
        Err(reason) => {
            return terminated(
                state.position().to_feen(),
                state.clocks(),
                Verdict::decisive(Status::IllegalMove, mover),
                Some(reason),
            );
        }
    };

    // 1b. Uchifuzume (ōgi): a Fu drop may not deliver checkmate. Resolved here,
    // above the `legality` layer, because the rule composes the terminal layer
    // (the opponent's legal-move search) over it — the same composition the
    // `engine` façade performs in `resolve_full`. `is_uchifuzume_drop` is inert
    // for any non-Fu drop, so the guard is a no-op outside ōgi Fu drops.
    if let Effect::Drop { piece, to } = effect {
        if is_uchifuzume_drop(state.position(), piece, to) {
            return terminated(
                state.position().to_feen(),
                state.clocks(),
                Verdict::decisive(Status::IllegalMove, mover),
                Some(IllegalReason::Uchifuzume),
            );
        }
    }

    // 2. Apply and canonicalize (defensive on the unreachable invariant break).
    let Ok(applied) = apply(state.position(), effect) else {
        return malformed(&state, mover);
    };
    let Ok(canonical) = canonicalize(&applied, &effect) else {
        return malformed(&state, mover);
    };

    // 3. Tick the mover's clock from the attestation-anchored elapsed time.
    let elapsed = attestation_at
        .duration_since(state.last_attestation())
        .unwrap_or(Duration::ZERO);
    let clocks = match tick(state.time_control(), state.clocks().get(mover), elapsed).clock() {
        Some(clock) => state.clocks().with(mover, clock),
        None => {
            // The mover flagged: a loss on time. The clocks reported are those
            // entering the ply (the mover's budget is spent regardless).
            return terminated(
                canonical.to_feen(),
                state.clocks(),
                Verdict::decisive(Status::Timeout, mover),
                None,
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
        terminated(feen, clocks, verdict, None)
    } else {
        StepResult {
            outcome: Outcome::ongoing(feen, clocks),
            next: Some(next_state),
        }
    }
}

/// Builds a terminated result (no successor state).
fn terminated(
    position: String,
    clocks: Clocks,
    verdict: Verdict,
    reason: Option<IllegalReason>,
) -> StepResult {
    StepResult {
        outcome: Outcome::new(position, clocks, verdict, reason),
        next: None,
    }
}

/// Defensive termination for an unreachable internal-invariant break.
fn malformed(state: &SessionState, mover: Side) -> StepResult {
    terminated(
        state.position().to_feen(),
        state.clocks(),
        Verdict::decisive(Status::IllegalMove, mover),
        Some(IllegalReason::Malformed),
    )
}

/// Whether the ply resets the 50-move half-move clock: a capture, or a board
/// move of an unpromoted pawn-class piece (read on the source position, so a
/// promotion — whose `placed` is the promoted piece — still counts). Per the
/// literal rule ("no capture and no unpromoted pawn-class move"), neither a drop
/// nor castling resets the clock.
#[inline]
fn resets_move_limit(position: &Position, effect: &Effect) -> bool {
    match effect {
        Effect::Board { from, captured, .. } => {
            captured.is_some() || position.piece_at(*from).is_some_and(Piece::is_foot_soldier)
        }
        Effect::Castle(_) | Effect::Drop { .. } => false,
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

    use super::step;
    use crate::domain::half_move::Move;
    use crate::domain::outcome::Verdict;
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

    #[test]
    fn legal_move_continues_and_increments_counter() {
        let result = step(
            state("4k^3/8/8/8/8/8/8/R3K^3 / W/w", 600),
            &mv("[\"a1\",\"a4\",null]"),
            Timestamp::from_unix(30),
        );
        assert_eq!(result.outcome.verdict, Verdict::Ongoing);
        let next = result.next.expect("the game continues");
        assert_eq!(next.half_move(), 2);
        // Quiet move (neither capture nor foot-soldier): the counter increments.
        assert_eq!(next.halfmove_clock(), 1);
    }

    #[test]
    fn illegal_move_terminates_game() {
        // No piece of the side to move on the source square.
        let result = step(
            state("4k^3/8/8/8/8/8/8/4K^3 / W/w", 600),
            &mv("[\"a1\",\"a4\",null]"),
            Timestamp::from_unix(30),
        );
        assert!(matches!(
            result.outcome.verdict,
            Verdict::Terminated {
                status: Status::IllegalMove,
                ..
            }
        ));
        assert!(result.outcome.reason.is_some());
        // Position unchanged, no next state.
        assert_eq!(result.outcome.position, "4k^3/8/8/8/8/8/8/4K^3 / W/w");
        assert!(result.next.is_none());
    }

    #[test]
    fn checkmate_terminates_game() {
        // Back-rank mate: Ra1→a8 mates the black King h8 walled in by its pawns.
        let result = step(
            state("7k^/6pp/8/8/8/8/8/R3K^3 / W/w", 600),
            &mv("[\"a1\",\"a8\",null]"),
            Timestamp::from_unix(30),
        );
        assert_eq!(
            result.outcome.verdict,
            Verdict::decisive(Status::Checkmate, Side::Second)
        );
        assert!(result.next.is_none());
    }

    #[test]
    fn timeout_terminates_game() {
        // 5 s bank, but 100 s elapsed: the side to move overruns.
        let result = step(
            state("4k^3/8/8/8/8/8/8/R3K^3 / W/w", 5),
            &mv("[\"a1\",\"a4\",null]"),
            Timestamp::from_unix(100),
        );
        assert_eq!(
            result.outcome.verdict,
            Verdict::decisive(Status::Timeout, Side::First)
        );
        assert!(result.next.is_none());
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
        assert_eq!(result.outcome.verdict, Verdict::Ongoing);
        let next = result.next.expect("the game continues");
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
        assert_eq!(result.outcome.verdict, Verdict::drawn(Status::Insufficient));
        assert!(result.next.is_none());
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
        assert_eq!(result.outcome.verdict, Verdict::Ongoing);
        assert!(result.next.is_some());
    }

    #[test]
    fn mating_fu_drop_is_illegal_uchifuzume() {
        // Ōgi: black King walled in on h8, white Rook g1 and white Knight f6
        // (which defends h7); the first player holds a Fu in hand. Dropping it on
        // h7 would deliver checkmate -> illegal drop (uchifuzume), game terminated
        // against the first player.
        let result = step(
            state("7k^/8/5N2/8/8/8/8/4K^1R1 F/ J/j", 600),
            &mv("[null,\"h7\",\"fu\"]"),
            Timestamp::from_unix(30),
        );
        assert_eq!(
            result.outcome.verdict,
            Verdict::decisive(Status::IllegalMove, Side::First)
        );
        assert_eq!(
            result.outcome.reason,
            Some(crate::domain::outcome::IllegalReason::Uchifuzume)
        );
        assert!(result.next.is_none());
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
        assert_eq!(result.outcome.verdict, Verdict::Ongoing);
        assert!(result.next.is_some());
    }
}
