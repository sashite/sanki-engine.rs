//! `Timestamp` and `Duration` — the time base, in whole seconds.
//!
//! All time accounting is anchored on the *canonical* timestamper attestation's
//! `created_at` (kind `1041`), never on a Ply's own `created_at` (which is purely
//! declarative) — see `6423-ply.md` § Time accounting and `race-resolution.md`.
//! The protocol's granularity is the **second** (Nostr `created_at` values are
//! integer Unix seconds), hence the absence of sub-second precision here.
//!
//! All arithmetic is *checked* (in line with `deny(arithmetic_side_effects)`):
//! an overflow or a negative subtraction returns `None` rather than panicking or
//! wrapping.

use core::fmt;

/// An instant, in Unix seconds — an attestation's `created_at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp(i64);

/// A non-negative duration, in whole seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Duration(u64);

impl Timestamp {
    /// Builds an instant from a number of Unix seconds.
    #[inline]
    #[must_use]
    pub const fn from_unix(secs: i64) -> Self {
        Self(secs)
    }

    /// The number of Unix seconds.
    #[inline]
    #[must_use]
    pub const fn as_unix(self) -> i64 {
        self.0
    }

    /// Elapsed duration between `earlier` and `self`.
    ///
    /// Returns `None` if `earlier` is after `self` (out-of-order instants) or if
    /// the difference overflows — so the caller must handle these cases
    /// explicitly rather than assuming monotonicity.
    #[inline]
    #[must_use]
    pub fn duration_since(self, earlier: Self) -> Option<Duration> {
        let delta = self.0.checked_sub(earlier.0)?;
        u64::try_from(delta).ok().map(Duration)
    }
}

impl Duration {
    /// The zero duration.
    pub const ZERO: Self = Self(0);

    /// Builds a duration from a number of seconds.
    #[inline]
    #[must_use]
    pub const fn from_secs(secs: u64) -> Self {
        Self(secs)
    }

    /// The number of seconds.
    #[inline]
    #[must_use]
    pub const fn as_secs(self) -> u64 {
        self.0
    }

    /// True if the duration is zero.
    #[inline]
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Bounded sum (e.g. adding the Fischer increment); `None` on overflow.
    #[inline]
    #[must_use]
    pub const fn checked_add(self, other: Self) -> Option<Self> {
        match self.0.checked_add(other.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Bounded subtraction (e.g. debiting the budget); `None` if the result would
    /// be negative — that is the signal of a time overrun (`timeout`).
    #[inline]
    #[must_use]
    pub const fn checked_sub(self, other: Self) -> Option<Self> {
        match self.0.checked_sub(other.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Subtraction saturated at zero (e.g. to display a remaining budget).
    #[inline]
    #[must_use]
    pub const fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}s", self.0)
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

    use super::{Duration, Timestamp};

    #[test]
    fn timestamp_round_trip() {
        assert_eq!(Timestamp::from_unix(1_700_000_000).as_unix(), 1_700_000_000);
        assert_eq!(Timestamp::from_unix(0).as_unix(), 0);
    }

    #[test]
    fn duration_since_respects_order() {
        let t1 = Timestamp::from_unix(1_700_000_030);
        let t2 = Timestamp::from_unix(1_700_000_100);
        assert_eq!(t2.duration_since(t1), Some(Duration::from_secs(70)));
        assert_eq!(t1.duration_since(t1), Some(Duration::ZERO));
        // earlier is later -> None (out of order).
        assert_eq!(t1.duration_since(t2), None);
    }

    #[test]
    fn duration_since_does_not_overflow() {
        // A difference that would overflow i64 -> None rather than a panic.
        let lo = Timestamp::from_unix(i64::MIN);
        let hi = Timestamp::from_unix(i64::MAX);
        assert_eq!(hi.duration_since(lo), None);
    }

    #[test]
    fn timestamp_orders() {
        assert!(Timestamp::from_unix(1) < Timestamp::from_unix(2));
        let mut v = [
            Timestamp::from_unix(3),
            Timestamp::from_unix(1),
            Timestamp::from_unix(2),
        ];
        v.sort();
        assert_eq!(
            v,
            [
                Timestamp::from_unix(1),
                Timestamp::from_unix(2),
                Timestamp::from_unix(3)
            ]
        );
    }

    #[test]
    fn duration_accessors() {
        assert_eq!(Duration::ZERO.as_secs(), 0);
        assert!(Duration::ZERO.is_zero());
        assert_eq!(Duration::from_secs(42).as_secs(), 42);
        assert!(!Duration::from_secs(1).is_zero());
    }

    #[test]
    fn duration_checked_add() {
        assert_eq!(
            Duration::from_secs(300).checked_add(Duration::from_secs(3)),
            Some(Duration::from_secs(303)),
        );
        assert_eq!(
            Duration::from_secs(u64::MAX).checked_add(Duration::from_secs(1)),
            None,
        );
    }

    #[test]
    fn duration_checked_sub() {
        assert_eq!(
            Duration::from_secs(300).checked_sub(Duration::from_secs(70)),
            Some(Duration::from_secs(230)),
        );
        assert_eq!(
            Duration::from_secs(5).checked_sub(Duration::from_secs(5)),
            Some(Duration::ZERO)
        );
        // Budget exceeded -> None (timeout signal).
        assert_eq!(
            Duration::from_secs(5).checked_sub(Duration::from_secs(6)),
            None
        );
    }

    #[test]
    fn duration_saturating_sub() {
        assert_eq!(
            Duration::from_secs(5).saturating_sub(Duration::from_secs(8)),
            Duration::ZERO
        );
        assert_eq!(
            Duration::from_secs(8).saturating_sub(Duration::from_secs(5)),
            Duration::from_secs(3),
        );
    }

    #[test]
    fn display() {
        assert_eq!(
            Timestamp::from_unix(1_700_000_000).to_string(),
            "1700000000"
        );
        assert_eq!(Duration::from_secs(120).to_string(), "120s");
    }
}
