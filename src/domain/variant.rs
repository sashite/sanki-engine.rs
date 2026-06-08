//! `Variant` and `VariantAssignment`.
//!
//! The three Sanki variants share a single 8×8 board, and each side plays its
//! own variant. A variant is selected by the SIN style letter (`W` → Chess,
//! `C` → Xiongqi, `J` → Ogi); applying that mapping belongs to the style layer,
//! while this module only defines the variant types the rest of the engine
//! consumes.

use crate::domain::side::Side;

/// One of the three Sanki variants played on the shared 8×8 board.
///
/// The set is closed by definition (*sanki* = "three games"), so it is
/// intentionally not `#[non_exhaustive]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variant {
    /// Western chess (style `W`).
    Chess,
    /// Ōgi — an 8×8 adaptation of shōgi (style `J`).
    Ogi,
    /// Xiongqi — an 8×8 adaptation of xiàngqí (style `C`).
    Xiongqi,
}

/// The variant assigned to each side for a given session.
///
/// The style is native to the side: uppercase tokens map to [`Side::First`],
/// lowercase tokens to [`Side::Second`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VariantAssignment {
    /// The first player's variant.
    pub first: Variant,
    /// The second player's variant.
    pub second: Variant,
}

impl VariantAssignment {
    /// Builds a variant assignment from each side's variant.
    #[inline]
    #[must_use]
    pub const fn new(first: Variant, second: Variant) -> Self {
        Self { first, second }
    }

    /// Returns the variant played by `side`.
    #[inline]
    #[must_use]
    pub const fn variant_of(self, side: Side) -> Variant {
        match side {
            Side::First => self.first,
            Side::Second => self.second,
        }
    }
}
