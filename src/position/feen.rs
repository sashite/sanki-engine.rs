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
    /// Number of dimensions of a Sanki board.
    const DIMENSIONS: usize = 2;

    /// Decodes a Sanki position from its FEEN string.
    ///
    /// # Errors
    /// - [`FeenError::Parse`] if the string is not valid FEEN;
    /// - [`FeenError::NotSankiBoard`] if the board is not an 8×8;
    /// - [`FeenError::Position`] if the styles are not Sanki styles.
    pub fn parse(feen: &str) -> Result<Self, FeenError> {
        let view = Feen::parse(feen).map_err(FeenError::Parse)?;

        // Sanki is strictly 8×8; our `Square` and `board` only address this
        // geometry. Reject any other board before materializing.
        let shape = view.shape();
        let dims = shape.dimensions();
        let is_8x8 =
            dims.len() == Self::DIMENSIONS && dims.iter().all(|&d| d == Square::FILE_COUNT);
        if !is_8x8 {
            return Err(FeenError::NotSankiBoard);
        }

        Self::new(view.to_qi()).map_err(FeenError::Position)
    }

    /// Re-encodes the position into canonical FEEN.
    #[inline]
    #[must_use]
    pub fn to_feen(&self) -> String {
        sashite_feen::encode(self.qi())
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
}
