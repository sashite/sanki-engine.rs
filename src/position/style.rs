//! Mapping between SIN style tokens and Sanki variants.
//!
//! A SIN token encodes (side, style) in a single letter: the **case** gives the
//! side (uppercase → `first`, lowercase → `second`) and the **letter** gives the
//! style. For Sanki, Sashité fixes the three conventional styles:
//!
//! | Letter | Style    | Variant |
//! |--------|----------|---------|
//! | `W`    | Western  | chess   |
//! | `C`    | Chinese  | xiongqi |
//! | `J`    | Japanese | ōgi     |
//!
//! The side is already resolved upstream: `Qi` stores the styles in `first_style`
//! / `second_style`, and the turn comes from `Qi::turn()`. This module therefore
//! only needs the **SIN → Variant** direction (the letter, case-insensitive).
//! Since the styles are fixed for the whole game, the SIN tokens are reused as-is
//! when encoding: no Variant → SIN reconstruction is needed.

use crate::domain::variant::{Variant, VariantAssignment};
use sashite_sin::Identifier as Sin;

/// The Sanki variant denoted by a style token, or `None` if the letter is not a
/// Sanki style (`W`/`C`/`J`).
#[inline]
#[must_use]
pub fn variant_of(style: Sin) -> Option<Variant> {
    match style.letter().as_char().to_ascii_uppercase() {
        'W' => Some(Variant::Chess),
        'C' => Some(Variant::Xiongqi),
        'J' => Some(Variant::Ogi),
        _ => None,
    }
}

/// The assignment of variants to both sides, from a position's `first_style` and
/// `second_style`.
///
/// The case ↔ side consistency (the first style uppercase, the second lowercase)
/// is guaranteed upstream by FEEN/`Qi`; here, only the letter matters.
///
/// # Errors
/// [`StyleError::UnknownStyle`] if either style is not a Sanki style.
pub fn assignment(first_style: Sin, second_style: Sin) -> Result<VariantAssignment, StyleError> {
    let first = variant_of(first_style)
        .ok_or_else(|| StyleError::UnknownStyle(first_style.letter().as_char()))?;
    let second = variant_of(second_style)
        .ok_or_else(|| StyleError::UnknownStyle(second_style.letter().as_char()))?;
    Ok(VariantAssignment::new(first, second))
}

/// A style-mapping error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleError {
    /// The style letter is not a Sanki style (`W`, `C`, or `J`).
    UnknownStyle(char),
}

impl core::fmt::Display for StyleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownStyle(c) => {
                write!(f, "style '{c}' unknown in Sanki (expected W, C, or J)")
            }
        }
    }
}

impl core::error::Error for StyleError {}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::{assignment, variant_of, StyleError};
    use crate::domain::variant::Variant;
    use sashite_sin::Identifier as Sin;

    fn sin(s: &str) -> Sin {
        Sin::parse(s).expect("valid SIN token")
    }

    #[test]
    fn variant_of_known_letters() {
        // Case-insensitive (the case is the side, not the style).
        assert_eq!(variant_of(sin("W")), Some(Variant::Chess));
        assert_eq!(variant_of(sin("w")), Some(Variant::Chess));
        assert_eq!(variant_of(sin("C")), Some(Variant::Xiongqi));
        assert_eq!(variant_of(sin("c")), Some(Variant::Xiongqi));
        assert_eq!(variant_of(sin("J")), Some(Variant::Ogi));
        assert_eq!(variant_of(sin("j")), Some(Variant::Ogi));
    }

    #[test]
    fn variant_of_unknown_letter() {
        assert_eq!(variant_of(sin("S")), None); // Siamese: not a Sanki variant
        assert_eq!(variant_of(sin("z")), None);
    }

    #[test]
    fn assignment_cross_variant() {
        // First player Western (W), second player Japanese (j).
        let a = assignment(sin("W"), sin("j")).expect("Sanki styles");
        assert_eq!(a.first, Variant::Chess);
        assert_eq!(a.second, Variant::Ogi);
    }

    #[test]
    fn assignment_same_variant() {
        let a = assignment(sin("C"), sin("c")).expect("Sanki styles");
        assert_eq!(a.first, Variant::Xiongqi);
        assert_eq!(a.second, Variant::Xiongqi);
    }

    #[test]
    fn assignment_unknown_style_rejected() {
        assert!(matches!(
            assignment(sin("W"), sin("s")),
            Err(StyleError::UnknownStyle(_)),
        ));
    }
}
