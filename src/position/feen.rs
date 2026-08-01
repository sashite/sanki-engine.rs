//! FEEN boundary: `Position::parse` decoding and `Position::to_feen` re-encoding.
//!
//! `Feen::parse` validates the string in one pass and returns a borrowed *view*;
//! `view.to_qi()` materializes it into `Qi<Epin, Sin>`; `sashite_feen::encode`
//! re-encodes a `Qi` into canonical FEEN (re-encoding an unchanged position
//! reproduces the input exactly). We additionally check that the board is indeed
//! an **8×8** — the only geometry Sanki (and our `Square`) addresses.

use super::{Position, PositionError};
use crate::domain::square::Square;
use sashite_feen::{Feen, ParseError};

/// Error decoding a Sanki FEEN position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeenError {
    /// The string is not valid FEEN.
    Parse(ParseError),
    /// The board is not an 8×8 (outside Sanki).
    NotSankiBoard,
    /// The FEEN is well-formed but its styles are not Sanki styles.
    Position(PositionError),
}

impl Position {
    /// Decodes a Sanki position from its FEEN string.
    ///
    /// # Errors
    /// - [`FeenError::Parse`] if the string is not valid FEEN;
    /// - [`FeenError::NotSankiBoard`] if the board is not an 8×8;
    /// - [`FeenError::Position`] if the styles are not Sanki styles.
    pub fn parse(feen: &str) -> Result<Self, FeenError> {
        let view = Feen::parse(feen).map_err(FeenError::Parse)?;

        // `Position::new` enforces the 8×8 geometry for every construction
        // path, so this check is not what makes the invariant hold. It is here
        // to reject *before* `to_qi()` materializes the board: FEEN accepts up
        // to 65 025 squares, and there is no reason to allocate a 255×255 board
        // only to discard it. The variant returned is the same either way.
        let dimensions = view.shape();
        let sizes = dimensions.dimensions();
        let is_8x8 =
            sizes.len() == Self::DIMENSIONS && sizes.iter().all(|&size| size == Square::FILE_COUNT);
        if !is_8x8 {
            return Err(FeenError::NotSankiBoard);
        }

        Self::new(view.to_qi()).map_err(|error| match error {
            PositionError::NotSankiBoard => FeenError::NotSankiBoard,
            style @ PositionError::Style(_) => FeenError::Position(style),
        })
    }

    /// Re-encodes the position into canonical FEEN.
    ///
    /// # Panics
    ///
    /// Never. `sashite_feen::encode` is fallible because `Qi` admits positions
    /// FEEN cannot spell — a geometry whose outer dimensions are not all at
    /// least 2, or a board too large to fit `sashite_feen::MAX_STRING_LENGTH`.
    /// A `Position` is neither: [`Position::new`] rejects anything that is not
    /// an 8×8 board, and 64 squares encode to well under 1 KiB even when every
    /// one of them is occupied by a four-byte token. The `expect` therefore
    /// documents an invariant this crate owns, rather than a hope about input.
    /// `to_feen_never_fails_on_any_reachable_position` pins it.
    #[inline]
    #[must_use]
    #[expect(
        clippy::expect_used,
        reason = "unreachable: Position::new guarantees an 8x8 board, which \
                  always has a canonical FEEN form well under the length cap"
    )]
    pub fn to_feen(&self) -> String {
        sashite_feen::encode(self.qi()).expect("an 8×8 Sanki position always encodes")
    }
}

impl core::fmt::Display for FeenError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "invalid FEEN: {e}"),
            Self::NotSankiBoard => f.write_str("the board is not an 8×8 Sanki board"),
            Self::Position(e) => write!(f, "{e}"),
        }
    }
}

impl core::error::Error for FeenError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Parse(e) => Some(e),
            Self::NotSankiBoard => None,
            Self::Position(e) => Some(e),
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

    use super::FeenError;
    use crate::domain::side::Side;
    use crate::domain::square::Square;
    use crate::domain::variant::Variant;
    use crate::position::Position;

    // Chess starting position in Sanki (rooks `-R`: castling right kept but not
    // immediately playable; pawns `+P`: double step available).
    const CHESS_START: &str =
        "-rnbqk^bn-r/+p+p+p+p+p+p+p+p/8/8/8/8/+P+P+P+P+P+P+P+P/-RNBQK^BN-R / W/w";

    #[test]
    fn round_trip_canonical() {
        let p = Position::parse(CHESS_START).expect("valid Sanki FEEN");
        assert_eq!(p.to_feen(), CHESS_START);
    }

    #[test]
    fn reads_starting_position() {
        let p = Position::parse(CHESS_START).expect("valid FEEN");
        assert_eq!(p.active_side(), Side::First); // uppercase `W`
        assert_eq!(p.variant_of(Side::First), Variant::Chess);
        assert_eq!(p.variant_of(Side::Second), Variant::Chess);
        let king = p
            .piece_at(Square::parse("e1").expect("e1"))
            .expect("white king on e1");
        assert!(king.is_royal());
        assert_eq!(king.side(), Side::First);
    }

    #[test]
    fn non_8x8_board_rejected() {
        // 9×9 shōgi FEEN: structurally valid but outside Sanki.
        let shogi = "lnsgk^gsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGK^GSNL / J/j";
        assert!(matches!(
            Position::parse(shogi),
            Err(FeenError::NotSankiBoard)
        ));
    }

    #[test]
    fn malformed_feen_rejected() {
        // Missing `hands` field.
        assert!(matches!(
            Position::parse("8/8/8/8/8/8/8/8 W/w"),
            Err(FeenError::Parse(_)),
        ));
    }

    #[test]
    fn non_sanki_style_rejected() {
        // Valid 8×8 board, but `Siamese` styles (S/s), unknown in Sanki.
        assert!(matches!(
            Position::parse("8/8/8/8/8/8/8/8 / S/s"),
            Err(FeenError::Position(_)),
        ));
    }

    /// The `expect` inside [`Position::to_feen`] rests on an implication:
    /// *whatever `Position::new` accepts, `sashite_feen::encode` can spell.*
    ///
    /// `encode` became fallible in `sashite-feen` 0.2 because `Qi` admits
    /// geometries FEEN has no notation for — any shape whose outer dimensions
    /// are not all at least 2 — and boards too large for its length cap. This
    /// asserts the implication directly over every geometry `Qi` will build in
    /// that neighbourhood, rather than trusting the argument in the doc
    /// comment. If `Position::new` were ever loosened, this fails before the
    /// `expect` can.
    #[test]
    fn to_feen_never_fails_on_any_reachable_position() {
        use crate::position::Position as P;
        use sashite_qi::Qi;

        let first = sashite_sin::Identifier::parse("W").expect("style W");
        let second = sashite_sin::Identifier::parse("w").expect("style w");

        let mut accepted = 0usize;
        for a in 1..=10usize {
            for b in 0..=10usize {
                let mut shape = vec![a];
                if b > 0 {
                    shape.push(b);
                }
                let Ok(qi) = Qi::new(&shape, first, second) else {
                    continue;
                };
                if P::new(qi.clone()).is_ok() {
                    accepted += 1;
                    assert!(
                        sashite_feen::encode(&qi).is_ok(),
                        "{shape:?} is accepted by Position::new but has no FEEN form"
                    );
                }
            }
        }
        assert_eq!(accepted, 1, "exactly one geometry is a Sanki board");

        // The full board is the densest case the length cap could ever see.
        let full = Position::parse(CHESS_START).expect("valid Sanki FEEN");
        assert!(sashite_feen::encode(full.qi()).is_ok());
        assert!(full.to_feen().len() < sashite_feen::MAX_STRING_LENGTH);
    }
}
