//! `TimeControl`, `Period`, `Clock`, `Clocks`.
//!
//! Sanki time control is **session-global and symmetric**: a single
//! configuration feeds both clocks. It consists of one or more sequential
//! **periods** (the `time_control` tag format from `6420-direct-challenge.md`:
//! `[duration, increment?, plies?]`):
//!
//! - `duration` alone        → a fixed time bank;
//! - `duration + increment`  → a Fischer increment after each move;
//! - `+ plies`               → a quota (byō-yomi / Canadian): the period covers
//!   `plies` moves, then resets;
//! - `duration = 0 + increment + plies` → per-move counting, with no bank.
//!
//! This module carries only the **data** and its validity; per-ply counting
//! (Fischer, quota reset, period transition, `timeout`) lives in [`crate::clock`].

use crate::domain::side::Side;
use crate::domain::time::Duration;

/// A time-control period.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    duration: Duration,
    increment: Option<Duration>,
    plies: Option<u32>,
}

/// A time-control configuration: one or more ordered periods.
///
/// Non-emptiness is guaranteed by construction (a first period is always
/// present).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeControl {
    first: Period,
    rest: Vec<Period>,
}

/// A player's clock state at a given instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    remaining: Duration,
    period: usize,
    plies_in_period: u32,
}

/// Both players' clocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clocks {
    first: Clock,
    second: Clock,
}

/// Error returned when building a time control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeControlError {
    /// No period supplied (at least one is required).
    NoPeriods,
    /// `plies` present while `increment` is absent (forbidden by the format).
    PliesWithoutIncrement,
    /// `plies` equal to zero (it must be strictly positive).
    ZeroPlies,
}

impl Period {
    /// Builds a period, validating the format's constraints.
    ///
    /// # Errors
    /// - [`TimeControlError::PliesWithoutIncrement`] if `plies` is supplied
    ///   without `increment`;
    /// - [`TimeControlError::ZeroPlies`] if `plies` is zero.
    pub fn new(
        duration: Duration,
        increment: Option<Duration>,
        plies: Option<u32>,
    ) -> Result<Self, TimeControlError> {
        if plies.is_some() && increment.is_none() {
            return Err(TimeControlError::PliesWithoutIncrement);
        }
        if matches!(plies, Some(0)) {
            return Err(TimeControlError::ZeroPlies);
        }
        Ok(Self {
            duration,
            increment,
            plies,
        })
    }

    /// The period's budget.
    #[inline]
    #[must_use]
    pub const fn duration(self) -> Duration {
        self.duration
    }

    /// The per-move Fischer increment, if any.
    #[inline]
    #[must_use]
    pub const fn increment(self) -> Option<Duration> {
        self.increment
    }

    /// The period's move quota, if any (≥ 1).
    #[inline]
    #[must_use]
    pub const fn plies(self) -> Option<u32> {
        self.plies
    }
}

impl TimeControl {
    /// Builds a time control from a first period and the rest.
    #[inline]
    #[must_use]
    pub fn new(first: Period, rest: Vec<Period>) -> Self {
        Self { first, rest }
    }

    /// Builds a time control from an ordered list of periods.
    ///
    /// # Errors
    /// [`TimeControlError::NoPeriods`] if the list is empty.
    pub fn from_periods(periods: Vec<Period>) -> Result<Self, TimeControlError> {
        let mut it = periods.into_iter();
        let first = it.next().ok_or(TimeControlError::NoPeriods)?;
        Ok(Self {
            first,
            rest: it.collect(),
        })
    }

    /// The first period (always present).
    #[inline]
    #[must_use]
    pub const fn first(&self) -> &Period {
        &self.first
    }

    /// The period at index `idx`, or `None` past the last one.
    #[inline]
    #[must_use]
    pub fn period(&self, idx: usize) -> Option<&Period> {
        match idx {
            0 => Some(&self.first),
            _ => self.rest.get(idx.checked_sub(1)?),
        }
    }

    /// Total number of periods (≥ 1).
    #[inline]
    #[must_use]
    pub fn period_count(&self) -> usize {
        self.rest.len().saturating_add(1)
    }
}

impl Clock {
    /// Initial clock state for the time control `tc`.
    #[inline]
    #[must_use]
    pub fn start(tc: &TimeControl) -> Self {
        Self {
            remaining: tc.first().duration(),
            period: 0,
            plies_in_period: 0,
        }
    }

    /// Builds an arbitrary clock state (used by [`crate::clock`] to produce the
    /// next state functionally).
    #[inline]
    #[must_use]
    pub const fn new(remaining: Duration, period: usize, plies_in_period: u32) -> Self {
        Self {
            remaining,
            period,
            plies_in_period,
        }
    }

    /// Remaining budget.
    #[inline]
    #[must_use]
    pub const fn remaining(self) -> Duration {
        self.remaining
    }

    /// Index of the current period.
    #[inline]
    #[must_use]
    pub const fn period_index(self) -> usize {
        self.period
    }

    /// Number of moves already played in the current period.
    #[inline]
    #[must_use]
    pub const fn plies_in_period(self) -> u32 {
        self.plies_in_period
    }
}

impl Clocks {
    /// Initial clocks (identical on both sides — the TC is symmetric).
    #[inline]
    #[must_use]
    pub fn start(tc: &TimeControl) -> Self {
        let c = Clock::start(tc);
        Self {
            first: c,
            second: c,
        }
    }

    /// The clock for `side`.
    #[inline]
    #[must_use]
    pub const fn get(self, side: Side) -> Clock {
        match side {
            Side::First => self.first,
            Side::Second => self.second,
        }
    }

    /// A copy where `side`'s clock is replaced by `clock`.
    #[inline]
    #[must_use]
    pub const fn with(self, side: Side, clock: Clock) -> Self {
        match side {
            Side::First => Self {
                first: clock,
                second: self.second,
            },
            Side::Second => Self {
                first: self.first,
                second: clock,
            },
        }
    }
}

impl core::fmt::Display for TimeControlError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let msg = match self {
            Self::NoPeriods => "at least one time-control period is required",
            Self::PliesWithoutIncrement => "`plies` requires `increment`",
            Self::ZeroPlies => "`plies` must be strictly positive",
        };
        f.write_str(msg)
    }
}

impl core::error::Error for TimeControlError {}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::{Clock, Clocks, Period, TimeControl, TimeControlError};
    use super::{Duration, Side};

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn period_accepts_valid_configs() {
        // duration only
        assert!(Period::new(secs(600), None, None).is_ok());
        // Fischer
        assert!(Period::new(secs(300), Some(secs(3)), None).is_ok());
        // quota
        assert!(Period::new(secs(0), Some(secs(30)), Some(1)).is_ok());
    }

    #[test]
    fn period_rejects_plies_without_increment() {
        assert_eq!(
            Period::new(secs(300), None, Some(1)),
            Err(TimeControlError::PliesWithoutIncrement),
        );
    }

    #[test]
    fn period_rejects_zero_plies() {
        assert_eq!(
            Period::new(secs(0), Some(secs(30)), Some(0)),
            Err(TimeControlError::ZeroPlies),
        );
    }

    #[test]
    fn period_accessors() {
        let p = Period::new(secs(300), Some(secs(3)), None).expect("valid period");
        assert_eq!(p.duration(), secs(300));
        assert_eq!(p.increment(), Some(secs(3)));
        assert_eq!(p.plies(), None);
    }

    #[test]
    fn time_control_rejects_empty() {
        assert_eq!(
            TimeControl::from_periods(Vec::new()),
            Err(TimeControlError::NoPeriods),
        );
    }

    #[test]
    fn time_control_multiple_periods() {
        // Mirrors Example 3 of 6420: ["time_control","3600"] then ["0","30","1"].
        let main = Period::new(secs(3600), None, None).expect("ok");
        let overtime = Period::new(secs(0), Some(secs(30)), Some(1)).expect("ok");
        let tc = TimeControl::from_periods(vec![main, overtime]).expect("ok");

        assert_eq!(tc.period_count(), 2);
        assert_eq!(tc.first(), &main);
        assert_eq!(tc.period(0), Some(&main));
        assert_eq!(tc.period(1), Some(&overtime));
        assert_eq!(tc.period(2), None);
    }

    #[test]
    fn clock_initial_state() {
        let main = Period::new(secs(3600), None, None).expect("ok");
        let tc = TimeControl::new(main, Vec::new());
        let c = Clock::start(&tc);
        assert_eq!(c.remaining(), secs(3600));
        assert_eq!(c.period_index(), 0);
        assert_eq!(c.plies_in_period(), 0);
    }

    #[test]
    fn clocks_symmetric_and_update() {
        let main = Period::new(secs(300), Some(secs(3)), None).expect("ok");
        let tc = TimeControl::new(main, Vec::new());
        let clocks = Clocks::start(&tc);
        assert_eq!(clocks.get(Side::First), clocks.get(Side::Second));

        let debited = Clock::new(secs(280), 0, 1);
        let updated = clocks.with(Side::First, debited);
        assert_eq!(updated.get(Side::First), debited);
        // The second side is unchanged.
        assert_eq!(updated.get(Side::Second), clocks.get(Side::Second));
    }
}
