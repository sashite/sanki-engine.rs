//! Per-ply time accounting: spend the elapsed time on the mover's clock, apply
//! the increment, advance or repeat quota periods, and detect time-out.
//!
//! The elapsed time of a ply is anchored on the canonical timestamper
//! attestations (kind `1041`): it is the difference between consecutive
//! canonical `created_at` values, anchored at t₀ — never a Ply's own
//! (declarative) `created_at`. Sequencing those attestations is the kernel's
//! job; this module is a **pure function** that takes the already-computed
//! [`Duration`] a ply consumed and the mover's current [`Clock`].
//!
//! # Time-control model
//!
//! A [`Period`](crate::domain::time_control::Period) is `duration` + optional `increment` + optional `plies` quota
//! (kind `6420` §time_control; PCN §8). The increment's timing differs by
//! configuration, which is the only thing that distinguishes the spend rule:
//!
//! - **Fixed bank / Fischer** (no `plies`): the ply MUST complete within
//!   `remaining`; the increment is a **post-ply bonus** (kind `6420`:
//!   "increment is added to the budget *after each ply*"). The flag falls when
//!   `elapsed > remaining`, and otherwise `remaining' = remaining − elapsed +
//!   increment`.
//! - **Quota** (`plies` present — byōyomi, Canadian, per-move): the increment is
//!   the per-ply **allowance, available during the ply**. The flag falls when
//!   `elapsed > remaining + increment`, and otherwise `remaining' = remaining +
//!   increment − elapsed`. This is forced by the `duration:0 + increment + plies`
//!   configuration ("per-move accounting with no main clock"): were the
//!   increment a post-ply bonus there, the very first move would flag.
//!
//! Two behaviours that the protocol specifies only at the semantic level (kind
//! `6420` / PCN §8 leave terminal evaluation to the rule system) are made
//! precise here:
//!
//! 1. **Period transition** — a `plies` quota, once reached, advances to the
//!    next period (its bank reset to that period's `duration`), or repeats the
//!    current period when it is the last (byōyomi/Canadian reset).
//! 2. **Bank exhaustion** — a period *without* a quota whose bank is exhausted
//!    **rolls over** into the next period, charging the overspend there
//!    (realizing "main bank followed by overtime"). The flag falls only when no
//!    further period remains, or when a quota period exhausts its allowance.

use crate::domain::time::Duration;
use crate::domain::time_control::{Clock, TimeControl};

/// Outcome of accounting one ply against the mover's clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tick {
    /// The mover stayed within budget; carries the updated clock.
    Continued(Clock),
    /// The mover exceeded the available budget — a loss on time.
    Flagged,
}

impl Tick {
    /// Whether the mover ran out of time.
    #[inline]
    #[must_use]
    pub const fn is_flagged(self) -> bool {
        matches!(self, Self::Flagged)
    }

    /// The updated clock, or `None` if the mover flagged.
    #[inline]
    #[must_use]
    pub const fn clock(self) -> Option<Clock> {
        match self {
            Self::Continued(clock) => Some(clock),
            Self::Flagged => None,
        }
    }
}

/// Accounts `elapsed` against `clock` under the time control `tc`, returning the
/// mover's updated clock or [`Tick::Flagged`] on time-out.
///
/// `clock` is the mover's clock at the start of their ply; `elapsed` is the time
/// the ply consumed, derived from the canonical attestation timestamps.
#[must_use]
pub fn tick(tc: &TimeControl, clock: Clock, elapsed: Duration) -> Tick {
    // A clock always points at an existing period; a stale index is treated as a
    // flag rather than trusted.
    let Some(period) = tc.period(clock.period_index()) else {
        return Tick::Flagged;
    };
    let increment = period.increment().unwrap_or(Duration::ZERO);

    match period.plies() {
        // Quota period: the increment is the per-ply allowance, available during
        // the ply.
        Some(quota) => {
            let available = add(clock.remaining(), increment);
            let Some(remaining_after) = available.checked_sub(elapsed) else {
                return Tick::Flagged;
            };
            let plies_after = clock.plies_in_period().saturating_add(1);
            if plies_after >= quota {
                // Quota reached: advance to the next period, or repeat the
                // current one when it is the last (bank reset to its duration).
                let next_index = clock.period_index().saturating_add(1);
                let (index, reset) = match tc.period(next_index) {
                    Some(next) => (next_index, next),
                    None => (clock.period_index(), period),
                };
                return Tick::Continued(Clock::new(reset.duration(), index, 0));
            }
            Tick::Continued(Clock::new(
                remaining_after,
                clock.period_index(),
                plies_after,
            ))
        }
        // Fixed bank or Fischer: the ply must complete within `remaining`; the
        // increment is a post-ply bonus.
        None => {
            let Some(remaining_after_spend) = clock.remaining().checked_sub(elapsed) else {
                // Bank exhausted: roll the overspend into the next period
                // (overtime), or flag if none remains.
                let next_index = clock.period_index().saturating_add(1);
                if let Some(next) = tc.period(next_index) {
                    let over = elapsed.saturating_sub(clock.remaining());
                    return tick(tc, Clock::new(next.duration(), next_index, 0), over);
                }
                return Tick::Flagged;
            };
            let remaining_after = add(remaining_after_spend, increment);
            let plies_after = clock.plies_in_period().saturating_add(1);
            Tick::Continued(Clock::new(
                remaining_after,
                clock.period_index(),
                plies_after,
            ))
        }
    }
}

/// Saturating sum of two durations (overflow is astronomically out of range for
/// second-granularity clocks; saturating keeps the function total).
#[inline]
fn add(a: Duration, b: Duration) -> Duration {
    Duration::from_secs(a.as_secs().saturating_add(b.as_secs()))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::{tick, Tick};
    use crate::domain::time::Duration;
    use crate::domain::time_control::{Clock, Period, TimeControl};

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    fn period(duration: u64, increment: Option<u64>, plies: Option<u32>) -> Period {
        Period::new(secs(duration), increment.map(secs), plies).expect("valid period")
    }

    fn tc(periods: Vec<Period>) -> TimeControl {
        TimeControl::from_periods(periods).expect("valid time control")
    }

    #[test]
    fn fischer_spend_and_increment() {
        // [300, +3]: 300 − 10 + 3 = 293, one ply played in the period.
        let control = tc(vec![period(300, Some(3), None)]);
        let result = tick(&control, Clock::new(secs(300), 0, 0), secs(10));
        assert_eq!(result, Tick::Continued(Clock::new(secs(293), 0, 1)));
    }

    #[test]
    fn fischer_flags_when_out_of_time() {
        // remaining 5 < 10 elapsed: flag.
        let control = tc(vec![period(300, Some(3), None)]);
        let result = tick(&control, Clock::new(secs(5), 0, 0), secs(10));
        assert_eq!(result, Tick::Flagged);
        assert!(result.is_flagged());
        assert_eq!(result.clock(), None);
    }

    #[test]
    fn fischer_increment_not_available_during_ply() {
        // remaining 5, +3 increment, 7 s elapsed: the increment being a *post-ply*
        // bonus, the ply does not fit in 5 s → flag (not a survivor at 1 s that an
        // increment available during the ply would have yielded).
        let control = tc(vec![period(300, Some(3), None)]);
        assert_eq!(
            tick(&control, Clock::new(secs(5), 0, 0), secs(7)),
            Tick::Flagged
        );
    }

    #[test]
    fn fixed_bank_depletes_linearly() {
        // [600] with no increment: 600 − 60 = 540.
        let control = tc(vec![period(600, None, None)]);
        let result = tick(&control, Clock::new(secs(600), 0, 0), secs(60));
        assert_eq!(result, Tick::Continued(Clock::new(secs(540), 0, 1)));
    }

    #[test]
    fn fixed_bank_terminal_flags() {
        // [600] terminal, 50 remaining < 60: flag (no following period).
        let control = tc(vec![period(600, None, None)]);
        assert_eq!(
            tick(&control, Clock::new(secs(50), 0, 0), secs(60)),
            Tick::Flagged
        );
    }

    #[test]
    fn byoyomi_per_move_resets() {
        // [0, +30, /1]: 30 s per move. 20 s elapsed → quota 1 reached → reset to
        // the duration (0), same period.
        let control = tc(vec![period(0, Some(30), Some(1))]);
        let result = tick(&control, Clock::new(secs(0), 0, 0), secs(20));
        assert_eq!(result, Tick::Continued(Clock::new(secs(0), 0, 0)));
        // 40 s > 30 s available → flag.
        assert_eq!(
            tick(&control, Clock::new(secs(0), 0, 0), secs(40)),
            Tick::Flagged
        );
    }

    #[test]
    fn quota_reached_advances_period() {
        // [5400, +30, /40] then [1800, +30]: on the 40th ply, move to period 1,
        // bank reset to 1800.
        let control = tc(vec![
            period(5400, Some(30), Some(40)),
            period(1800, Some(30), None),
        ]);
        let result = tick(&control, Clock::new(secs(1000), 0, 39), secs(10));
        assert_eq!(result, Tick::Continued(Clock::new(secs(1800), 1, 0)));
    }

    #[test]
    fn rollover_from_main_bank_to_overtime() {
        // [3600] then [0, +30, /1]: 10 s remaining in the main bank, 25 s elapsed.
        // The overspend (15 s) is charged to the overtime (30 s available) → we
        // enter period 1, quota 1 reached → reset.
        let control = tc(vec![period(3600, None, None), period(0, Some(30), Some(1))]);
        let result = tick(&control, Clock::new(secs(10), 0, 0), secs(25));
        assert_eq!(result, Tick::Continued(Clock::new(secs(0), 1, 0)));
    }

    #[test]
    fn rollover_then_flag_if_overtime_insufficient() {
        // Same TC, but 50 s elapsed: overspend 40 s > 30 s overtime → flag.
        let control = tc(vec![period(3600, None, None), period(0, Some(30), Some(1))]);
        assert_eq!(
            tick(&control, Clock::new(secs(10), 0, 0), secs(50)),
            Tick::Flagged
        );
    }
}
