//! `SessionState` — the kernel's per-ply state (L1).
//!
//! The kernel is a pure per-ply function; `SessionState` is what it threads from
//! one ply to the next. It separates the session-**constant** configuration from
//! the **per-ply** state:
//!
//! - constant: the [`TimeControl`] (the per-side variants are carried by the
//!   [`Position`] itself, via its style field, and never change);
//! - per-ply: the canonical [`Position`], both [`Clocks`], the anchor
//!   [`Timestamp`] for the next ply's elapsed time (t₀ initially), the FEEN
//!   occurrence history (threefold repetition), the half-move clock (50-move
//!   rule), and the 1-based play-order position of the next ply.
//!
//! The terminal facts that depend on history rather than on the board alone are
//! exposed here for [`crate::terminal::classify`]: [`SessionState::threefold_repetition`],
//! [`SessionState::move_limit_reached`], and [`SessionState::move_cap_reached`]. Their
//! thresholds — and the
//! half-move-clock reset rule the step applies — come from
//! [`crate::terminal::repetition`], [`crate::terminal::move_limit`], and
//! [`crate::terminal::move_cap`], the
//! single sources of those rules; this type only owns the cross-ply
//! bookkeeping (an occurrence-count map rather than the modules' list-based
//! helpers, same semantics).
//!
//! [`SessionState::advance`] consumes the state to produce the next one, moving
//! the time control and history without cloning. The elapsed-time computation
//! and the move legality / canonicalization belong to the step orchestration,
//! which feeds `advance` the already-canonical next position, the ticked clocks,
//! the ply's attestation timestamp, and whether the ply was irreversible (a
//! capture or an unpromoted pawn-class move, per the rule system) for the
//! half-move clock.

use crate::domain::time::Timestamp;
use crate::domain::time_control::{Clocks, TimeControl};
use crate::position::Position;
use crate::terminal::{move_cap, move_limit, repetition};
use std::collections::HashMap;

/// The kernel's state between two plies.
#[derive(Debug, Clone)]
pub struct SessionState {
    position: Position,
    clocks: Clocks,
    time_control: TimeControl,
    last_attestation: Timestamp,
    history: HashMap<String, u32>,
    repetition_count: u32,
    halfmove_clock: u32,
    half_move: u32,
}

impl SessionState {
    /// The initial state of a session: clocks started from `time_control`, the
    /// half-move clock at zero, the next ply at play-order position 1, and the
    /// FEEN history seeded with the starting position. `anchor` is t₀, the
    /// canonical session-start timestamp against which the first ply's elapsed
    /// time is measured.
    #[must_use]
    pub fn start(position: Position, time_control: TimeControl, anchor: Timestamp) -> Self {
        let clocks = Clocks::start(&time_control);
        let mut history = HashMap::new();
        history.insert(position.to_feen(), 1);
        Self {
            position,
            clocks,
            time_control,
            last_attestation: anchor,
            history,
            repetition_count: 1,
            halfmove_clock: 0,
            half_move: 1,
        }
    }

    /// The current canonical position.
    #[inline]
    #[must_use]
    pub const fn position(&self) -> &Position {
        &self.position
    }

    /// Both players' clocks.
    #[inline]
    #[must_use]
    pub const fn clocks(&self) -> Clocks {
        self.clocks
    }

    /// The session's time control.
    #[inline]
    #[must_use]
    pub const fn time_control(&self) -> &TimeControl {
        &self.time_control
    }

    /// The anchor timestamp for the next ply's elapsed time (t₀ before the first
    /// ply, then each canonical attestation's `created_at`).
    #[inline]
    #[must_use]
    pub const fn last_attestation(&self) -> Timestamp {
        self.last_attestation
    }

    /// The 1-based play-order position of the next ply to be played (the count
    /// of half-moves so far, plus one). Distinct from a Ply's kind-`6423` `step`,
    /// which is each signer's own move ordinal; the mapping between the two is
    /// the consuming application's concern.
    #[inline]
    #[must_use]
    pub const fn half_move(&self) -> u32 {
        self.half_move
    }

    /// Plies elapsed since the last capture or unpromoted pawn-class move.
    #[inline]
    #[must_use]
    pub const fn halfmove_clock(&self) -> u32 {
        self.halfmove_clock
    }

    /// Whether the current position has now occurred three times (threefold
    /// repetition).
    #[inline]
    #[must_use]
    pub const fn threefold_repetition(&self) -> bool {
        self.repetition_count >= repetition::THREEFOLD as u32
    }

    /// Whether the 50-move rule's half-move threshold has been reached.
    #[inline]
    #[must_use]
    pub fn move_limit_reached(&self) -> bool {
        move_limit::limit_reached(self.halfmove_clock)
    }

    /// Whether the absolute 600-half-move cap has been reached — the `movecap`
    /// draw, when the position is otherwise still ongoing (Sanki global rule).
    #[inline]
    #[must_use]
    pub fn move_cap_reached(&self) -> bool {
        move_cap::cap_reached(self.half_move.saturating_sub(1))
    }

    /// Produces the next state after a legal ply.
    ///
    /// `position` is the canonical position the ply reaches, `clocks` the ticked
    /// clocks, `attestation_at` the ply's canonical attestation timestamp (the
    /// next anchor), and `irreversible` whether the ply reset the half-move clock
    /// (a capture or an unpromoted pawn-class move). The state is consumed: the
    /// time control and history move into the successor without cloning.
    #[must_use]
    pub fn advance(
        mut self,
        position: Position,
        clocks: Clocks,
        attestation_at: Timestamp,
        irreversible: bool,
    ) -> Self {
        let repetition_count = *self
            .history
            .entry(position.to_feen())
            .and_modify(|count| *count = count.saturating_add(1))
            .or_insert(1);

        let halfmove_clock = if irreversible {
            0
        } else {
            self.halfmove_clock.saturating_add(1)
        };
        let half_move = self.half_move.saturating_add(1);

        Self {
            position,
            clocks,
            time_control: self.time_control,
            last_attestation: attestation_at,
            history: self.history,
            repetition_count,
            halfmove_clock,
            half_move,
        }
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

    use super::SessionState;
    use crate::domain::time::{Duration, Timestamp};
    use crate::domain::time_control::{Clocks, Period, TimeControl};
    use crate::position::Position;

    fn pos(feen: &str) -> Position {
        Position::parse(feen).expect("valid Sanki FEEN")
    }

    fn time_control() -> TimeControl {
        let period = Period::new(Duration::from_secs(60), None, None).expect("valid period");
        TimeControl::new(period, Vec::new())
    }

    fn ts(secs: i64) -> Timestamp {
        Timestamp::from_unix(secs)
    }

    const START_FEEN: &str = "4k^3/8/8/8/8/8/8/4K^3 / W/w";

    #[test]
    fn start_initializes_the_state() {
        let tc = time_control();
        let expected_clocks = Clocks::start(&tc);
        let state = SessionState::start(pos(START_FEEN), tc, ts(1000));

        assert_eq!(state.half_move(), 1);
        assert_eq!(state.halfmove_clock(), 0);
        assert_eq!(state.last_attestation(), ts(1000));
        assert_eq!(state.clocks(), expected_clocks);
        assert!(!state.threefold_repetition());
        assert!(!state.move_limit_reached());
        assert_eq!(state.position().to_feen(), START_FEEN);
    }

    #[test]
    fn advance_increments_half_move_and_counter() {
        let state = SessionState::start(pos(START_FEEN), time_control(), ts(1000));
        let next_feen = "3k^4/8/8/8/8/8/8/4K^3 / w/W";
        let clocks = state.clocks();

        // Reversible move: the half-move clock increments.
        let next = state.advance(pos(next_feen), clocks, ts(1030), false);
        assert_eq!(next.half_move(), 2);
        assert_eq!(next.halfmove_clock(), 1);
        assert_eq!(next.last_attestation(), ts(1030));
        assert_eq!(next.position().to_feen(), next_feen);
    }

    #[test]
    fn advance_irreversible_resets_the_counter() {
        let state = SessionState::start(pos(START_FEEN), time_control(), ts(1000));
        let other = "3k^4/8/8/8/8/8/8/4K^3 / w/W";
        let clocks = state.clocks();

        // Two reversible moves bring the counter to 2…
        let state = state.advance(pos(other), clocks, ts(1010), false);
        let state = state.advance(pos(START_FEEN), clocks, ts(1020), false);
        assert_eq!(state.halfmove_clock(), 2);

        // …then an irreversible move (capture / foot-soldier) resets it to zero.
        let state = state.advance(pos(other), clocks, ts(1030), true);
        assert_eq!(state.halfmove_clock(), 0);
    }

    #[test]
    fn move_limit_reached_at_a_hundred_plies() {
        let mut state = SessionState::start(pos(START_FEEN), time_control(), ts(0));
        let clocks = state.clocks();
        let other = "3k^4/8/8/8/8/8/8/4K^3 / w/W";

        // 99 reversible plies: threshold not reached.
        for i in 0..99 {
            let feen = if i % 2 == 0 { other } else { START_FEEN };
            state = state.advance(pos(feen), clocks, ts(i64::from(i)), false);
        }
        assert_eq!(state.halfmove_clock(), 99);
        assert!(!state.move_limit_reached());

        // The 100th reaches the 50-move rule's threshold.
        state = state.advance(pos(other), clocks, ts(100), false);
        assert_eq!(state.halfmove_clock(), 100);
        assert!(state.move_limit_reached());
    }

    #[test]
    fn threefold_at_the_third_occurrence() {
        // The starting position counts as the 1st occurrence.
        let state = SessionState::start(pos(START_FEEN), time_control(), ts(0));
        let clocks = state.clocks();

        // 2nd occurrence of the same position: not yet a threefold repetition.
        let state = state.advance(pos(START_FEEN), clocks, ts(10), false);
        assert!(!state.threefold_repetition());

        // 3rd occurrence: threefold repetition.
        let state = state.advance(pos(START_FEEN), clocks, ts(20), false);
        assert!(state.threefold_repetition());
    }

    #[test]
    fn move_cap_reached_at_600_plies() {
        let mut state = SessionState::start(pos(START_FEEN), time_control(), ts(0));
        let clocks = state.clocks();
        let other = "3k^4/8/8/8/8/8/8/4K^3 / w/W";
        // 599 plies played: the absolute cap is not yet reached.
        for i in 0..599 {
            let feen = if i % 2 == 0 { other } else { START_FEEN };
            state = state.advance(pos(feen), clocks, ts(i64::from(i)), true);
        }
        assert!(!state.move_cap_reached());
        // The 600th ply reaches the 600-half-move cap.
        state = state.advance(pos(other), clocks, ts(600), true);
        assert!(state.move_cap_reached());
    }
}
