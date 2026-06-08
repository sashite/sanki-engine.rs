//! `Move` — a Sanki half-move — and parsing of a Ply's `content`.
//!
//! The type is named `Move` (the module is `half_move`, because `move` is a
//! reserved keyword). A Ply's `content` (kind `6423`, context `sanki`) is the
//! JSON triple `[source, destination, actor]` described in
//! `move-encoding-sanki.md`:
//!
//! - **source**: `null` (placement from hand) or a **square**;
//! - **destination**: always a **square**;
//! - **actor**: `null`, or a **piece-name** (`^[a-z]{1,16}$`) — non-`null` for a
//!   drop, or for a choice transformation.
//!
//! Parsing is purely **structural**: it checks the shape of the triple and its
//! sub-formats (square via [`Square`], piece-name via [`ActorName`]), but
//! evaluates no legality (actor relevance, reachability…), which is left to
//! `legality`. A non-`null` source and destination must differ.

use crate::domain::actor::ActorName;
use crate::domain::square::Square;
use serde_json::Value;

/// A Sanki half-move, as decoded from a Ply's `content`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Move {
    /// A move on the board: from `from` to `to`, with an optional transformation
    /// target (`actor`).
    Board {
        /// Origin square.
        from: Square,
        /// Destination square.
        to: Square,
        /// Chosen transformation target, if any.
        actor: Option<ActorName>,
    },
    /// A placement (drop) from hand: the piece `piece` is placed on `to`.
    Drop {
        /// Type of the placed piece.
        piece: ActorName,
        /// Destination square.
        to: Square,
    },
}

/// Error returned when parsing a Sanki Ply's `content`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveError {
    /// The `content` exceeds kind `6423`'s 256-character bound.
    TooLong,
    /// The `content` is not valid JSON.
    NotJson,
    /// The JSON is not an array.
    NotArray,
    /// The array does not have exactly three elements.
    WrongLength,
    /// The source is neither `null` nor a valid square.
    BadSource,
    /// The destination is not a valid square.
    BadDestination,
    /// The actor is neither `null` nor a valid piece-name.
    BadActor,
    /// Source and destination are identical.
    SourceEqualsDestination,
    /// A drop (source `null`) without an actor to name the placed piece.
    DropWithoutPiece,
}

impl Move {
    /// The `content` length bound set by kind `6423`.
    const MAX_CONTENT_LEN: usize = 256;

    /// Parses a Sanki Ply's `content`.
    ///
    /// # Errors
    /// Returns the [`MoveError`] describing the first defect encountered.
    pub fn parse(content: &str) -> Result<Self, MoveError> {
        if content.len() > Self::MAX_CONTENT_LEN {
            return Err(MoveError::TooLong);
        }
        let value: Value = serde_json::from_str(content).map_err(|_| MoveError::NotJson)?;
        let elements = match &value {
            Value::Array(elements) => elements,
            _ => return Err(MoveError::NotArray),
        };
        match elements.as_slice() {
            [source, destination, actor] => Self::from_elements(source, destination, actor),
            _ => Err(MoveError::WrongLength),
        }
    }

    /// Assembles a move from the three already-extracted JSON elements.
    fn from_elements(
        source: &Value,
        destination: &Value,
        actor: &Value,
    ) -> Result<Self, MoveError> {
        let to = match destination {
            Value::String(s) => Square::parse(s.as_str()).map_err(|_| MoveError::BadDestination)?,
            _ => return Err(MoveError::BadDestination),
        };

        let actor = match actor {
            Value::Null => None,
            Value::String(s) => {
                Some(ActorName::parse(s.as_str()).map_err(|_| MoveError::BadActor)?)
            }
            _ => return Err(MoveError::BadActor),
        };

        match source {
            // Null source: placement from hand — the actor names the piece.
            Value::Null => {
                let piece = actor.ok_or(MoveError::DropWithoutPiece)?;
                Ok(Self::Drop { piece, to })
            }
            // Present source: a move on the board.
            Value::String(s) => {
                let from = Square::parse(s.as_str()).map_err(|_| MoveError::BadSource)?;
                if from == to {
                    return Err(MoveError::SourceEqualsDestination);
                }
                Ok(Self::Board { from, to, actor })
            }
            _ => Err(MoveError::BadSource),
        }
    }

    /// True if this is a placement (drop).
    #[inline]
    #[must_use]
    pub const fn is_drop(&self) -> bool {
        matches!(self, Self::Drop { .. })
    }

    /// The destination square, common to both move forms.
    #[inline]
    #[must_use]
    pub const fn destination(&self) -> Square {
        match self {
            Self::Board { to, .. } | Self::Drop { to, .. } => *to,
        }
    }
}

impl core::fmt::Display for MoveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let msg = match self {
            Self::TooLong => "content exceeds 256 characters",
            Self::NotJson => "content is not valid JSON",
            Self::NotArray => "content is not a JSON array",
            Self::WrongLength => "the array does not have exactly three elements",
            Self::BadSource => "invalid source (neither null nor a square)",
            Self::BadDestination => "invalid destination (a square is expected)",
            Self::BadActor => "invalid actor (neither null nor a piece-name)",
            Self::SourceEqualsDestination => "source and destination are identical",
            Self::DropWithoutPiece => "drop without a named piece",
        };
        f.write_str(msg)
    }
}

impl core::error::Error for MoveError {}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::{Move, MoveError};
    use crate::domain::actor::ActorName;
    use crate::domain::square::Square;

    fn sq(s: &str) -> Square {
        Square::parse(s).expect("valid square")
    }

    fn actor(s: &str) -> ActorName {
        ActorName::parse(s).expect("valid name")
    }

    #[test]
    fn simple_move() {
        assert_eq!(
            Move::parse(r#"["a2","a4",null]"#),
            Ok(Move::Board {
                from: sq("a2"),
                to: sq("a4"),
                actor: None,
            }),
        );
    }

    #[test]
    fn move_with_transformation() {
        assert_eq!(
            Move::parse(r#"["g7","g8","queen"]"#),
            Ok(Move::Board {
                from: sq("g7"),
                to: sq("g8"),
                actor: Some(actor("queen")),
            }),
        );
    }

    #[test]
    fn simple_drop() {
        assert_eq!(
            Move::parse(r#"[null,"e5","fu"]"#),
            Ok(Move::Drop {
                piece: actor("fu"),
                to: sq("e5"),
            }),
        );
    }

    #[test]
    fn drop_without_piece_rejected() {
        assert_eq!(
            Move::parse(r#"[null,"e5",null]"#),
            Err(MoveError::DropWithoutPiece),
        );
    }

    #[test]
    fn source_equals_destination_rejected() {
        assert_eq!(
            Move::parse(r#"["a2","a2",null]"#),
            Err(MoveError::SourceEqualsDestination),
        );
    }

    #[test]
    fn malformed_shapes() {
        assert_eq!(Move::parse(""), Err(MoveError::NotJson));
        assert_eq!(Move::parse("{}"), Err(MoveError::NotArray));
        assert_eq!(Move::parse(r#"["a2","a4"]"#), Err(MoveError::WrongLength));
        assert_eq!(
            Move::parse(r#"["a2","a4",null,null]"#),
            Err(MoveError::WrongLength)
        );
        // Wrong types.
        assert_eq!(Move::parse(r#"[1,"a4",null]"#), Err(MoveError::BadSource));
        assert_eq!(
            Move::parse(r#"["a2",null,null]"#),
            Err(MoveError::BadDestination)
        );
        assert_eq!(Move::parse(r#"["a2","a4",5]"#), Err(MoveError::BadActor));
    }

    #[test]
    fn off_board_squares_rejected() {
        // z9 has the general shape of a square but lies outside the 8×8 board a1–h8.
        assert_eq!(
            Move::parse(r#"["z9","a4",null]"#),
            Err(MoveError::BadSource)
        );
        assert_eq!(
            Move::parse(r#"["a2","i9",null]"#),
            Err(MoveError::BadDestination)
        );
    }

    #[test]
    fn invalid_piece_name_rejected() {
        // 17 characters: beyond the piece-name bound.
        assert_eq!(
            Move::parse(r#"[null,"e5","abcdefghijklmnopq"]"#),
            Err(MoveError::BadActor),
        );
    }

    #[test]
    fn accessors() {
        let m = Move::parse(r#"[null,"e5","fu"]"#).expect("ok");
        assert!(m.is_drop());
        assert_eq!(m.destination(), sq("e5"));

        let b = Move::parse(r#"["a2","a4",null]"#).expect("ok");
        assert!(!b.is_drop());
        assert_eq!(b.destination(), sq("a4"));
    }
}
