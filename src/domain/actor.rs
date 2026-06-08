//! `ActorName` — the **piece-name** carried by a move's "actor" field — and the
//! `(Variant, ActorName) → base letter` table.
//!
//! In the `[source, destination, actor]` encoding (see `move-encoding-sanki.md`),
//! the third element is non-`null` in exactly two cases:
//! - **drop** (source `null`): it names the piece being placed;
//! - **choice transformation**: it names the chosen promotion target.
//!
//! The piece-name format is `^[a-z]{1,16}$`. The recognized vocabulary is
//! variant-specific, and coincides with the actor's single use for that variant:
//! - **chess** — promotion targets: queen, rook, bishop, knight;
//! - **ōgi** — drop pieces: fu, rook, bishop, knight, princess
//!   (the Fu→Tokin promotion is automatic and unique, hence carries no actor);
//! - **xiongqi** — promotion targets: chariot, knight, bear, empress.
//!
//! The table returns the **base letter** (uppercase); case, which encodes the
//! side, is applied by the caller (`legality`/`apply`) according to the player
//! to move. A name outside the variant's vocabulary yields `None`, which makes
//! the move illegal in its context.

use crate::domain::variant::Variant;

/// The piece-name carried by a move's "actor" field (`^[a-z]{1,16}$`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActorName(String);

/// Error returned when validating a piece-name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorNameError {
    /// Length outside the 1–16 range.
    Length,
    /// A character outside `[a-z]`.
    Character,
}

impl ActorName {
    /// Validates and builds an actor name from its textual form.
    ///
    /// # Errors
    /// - [`ActorNameError::Length`] if the length is not within 1–16;
    /// - [`ActorNameError::Character`] if a character is not in `[a-z]`.
    pub fn parse(s: &str) -> Result<Self, ActorNameError> {
        let bytes = s.as_bytes();
        if !(1..=16).contains(&bytes.len()) {
            return Err(ActorNameError::Length);
        }
        if !bytes.iter().all(u8::is_ascii_lowercase) {
            return Err(ActorNameError::Character);
        }
        Ok(Self(s.to_owned()))
    }

    /// The textual form of the name.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The base letter of the piece designated by this name in `variant`, or
    /// `None` if the name is not part of that variant's vocabulary.
    ///
    /// Matching is variant-first (exhaustive over the enum: adding a variant
    /// would force this table to be completed), then name.
    #[must_use]
    pub fn letter_for(&self, variant: Variant) -> Option<char> {
        match variant {
            Variant::Chess => match self.0.as_str() {
                "queen" => Some('Q'),
                "rook" => Some('R'),
                "bishop" => Some('B'),
                "knight" => Some('N'),
                _ => None,
            },
            Variant::Ogi => match self.0.as_str() {
                "fu" => Some('F'),
                "rook" => Some('R'),
                "bishop" => Some('B'),
                "knight" => Some('N'),
                "princess" => Some('I'),
                _ => None,
            },
            Variant::Xiongqi => match self.0.as_str() {
                "chariot" => Some('R'),
                "knight" => Some('N'),
                "bear" => Some('B'),
                "empress" => Some('E'),
                _ => None,
            },
        }
    }
}

impl core::str::FromStr for ActorName {
    type Err = ActorNameError;
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl core::fmt::Display for ActorName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::fmt::Display for ActorNameError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let msg = match self {
            Self::Length => "a piece-name must be 1 to 16 characters long",
            Self::Character => "a piece-name may only contain the letters [a-z]",
        };
        f.write_str(msg)
    }
}

impl core::error::Error for ActorNameError {}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::{ActorName, ActorNameError};
    use crate::domain::variant::Variant;

    fn name(s: &str) -> ActorName {
        ActorName::parse(s).expect("valid name")
    }

    #[test]
    fn parse_accepts_valid() {
        assert_eq!(name("queen").as_str(), "queen");
        assert_eq!(name("empress").as_str(), "empress");
        assert!(ActorName::parse("abcdefghijklmnop").is_ok()); // 16 characters
    }

    #[test]
    fn parse_rejects_length() {
        assert_eq!(ActorName::parse(""), Err(ActorNameError::Length));
        assert_eq!(
            ActorName::parse("abcdefghijklmnopq"), // 17 characters
            Err(ActorNameError::Length),
        );
    }

    #[test]
    fn parse_rejects_characters() {
        assert_eq!(ActorName::parse("Queen"), Err(ActorNameError::Character));
        assert_eq!(ActorName::parse("a1"), Err(ActorNameError::Character));
        assert_eq!(ActorName::parse("knight!"), Err(ActorNameError::Character));
    }

    #[test]
    fn table_chess() {
        assert_eq!(name("queen").letter_for(Variant::Chess), Some('Q'));
        assert_eq!(name("rook").letter_for(Variant::Chess), Some('R'));
        assert_eq!(name("bishop").letter_for(Variant::Chess), Some('B'));
        assert_eq!(name("knight").letter_for(Variant::Chess), Some('N'));
        // Outside the chess vocabulary.
        assert_eq!(name("king").letter_for(Variant::Chess), None);
        assert_eq!(name("pawn").letter_for(Variant::Chess), None);
        assert_eq!(name("empress").letter_for(Variant::Chess), None);
        assert_eq!(name("bear").letter_for(Variant::Chess), None);
    }

    #[test]
    fn table_ogi() {
        assert_eq!(name("fu").letter_for(Variant::Ogi), Some('F'));
        assert_eq!(name("rook").letter_for(Variant::Ogi), Some('R'));
        assert_eq!(name("bishop").letter_for(Variant::Ogi), Some('B'));
        assert_eq!(name("knight").letter_for(Variant::Ogi), Some('N'));
        assert_eq!(name("princess").letter_for(Variant::Ogi), Some('I'));
        // Outside the ōgi vocabulary.
        assert_eq!(name("queen").letter_for(Variant::Ogi), None);
        assert_eq!(name("chariot").letter_for(Variant::Ogi), None);
        assert_eq!(name("tokin").letter_for(Variant::Ogi), None); // a captured Tokin is held as Fu
    }

    #[test]
    fn table_xiongqi() {
        assert_eq!(name("chariot").letter_for(Variant::Xiongqi), Some('R'));
        assert_eq!(name("knight").letter_for(Variant::Xiongqi), Some('N'));
        assert_eq!(name("bear").letter_for(Variant::Xiongqi), Some('B'));
        assert_eq!(name("empress").letter_for(Variant::Xiongqi), Some('E'));
        // Outside the xiongqi vocabulary.
        assert_eq!(name("queen").letter_for(Variant::Xiongqi), None);
        assert_eq!(name("soldier").letter_for(Variant::Xiongqi), None);
        assert_eq!(name("general").letter_for(Variant::Xiongqi), None);
        assert_eq!(name("rook").letter_for(Variant::Xiongqi), None); // xiongqi calls it "chariot"
    }

    #[test]
    fn unknown_name_everywhere() {
        let u = name("unicorn");
        assert_eq!(u.letter_for(Variant::Chess), None);
        assert_eq!(u.letter_for(Variant::Ogi), None);
        assert_eq!(u.letter_for(Variant::Xiongqi), None);
    }
}
