//! `Side` (First/Second) and conversions with the notation layer.
//!
//! A piece's *side* is carried by the case of its EPIN token; this type is the
//! domain view of that information, independent of the side types exposed by the
//! notation crates (`sashite_epin::Side`, `sashite_sin::Side`).

/// The side a player / piece belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// First player (uppercase tokens).
    First,
    /// Second player (lowercase tokens).
    Second,
}

impl Side {
    /// Returns the opposing side.
    #[inline]
    #[must_use]
    pub const fn flip(self) -> Self {
        match self {
            Self::First => Self::Second,
            Self::Second => Self::First,
        }
    }
}

// `sashite_epin::Side` re-exports `sashite_pin::Side`, so these two conversions
// cover the PIN side type as well.
impl From<sashite_epin::Side> for Side {
    #[inline]
    fn from(value: sashite_epin::Side) -> Self {
        match value {
            sashite_epin::Side::First => Self::First,
            sashite_epin::Side::Second => Self::Second,
        }
    }
}

impl From<Side> for sashite_epin::Side {
    #[inline]
    fn from(value: Side) -> Self {
        match value {
            Side::First => Self::First,
            Side::Second => Self::Second,
        }
    }
}
