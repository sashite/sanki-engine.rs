//! Position model: a wrapper around `sashite_qi::Qi<Epin, Sin>`.
//!
//! [`Position`] fixes the type parameters (EPIN pieces, SIN styles) and offers a
//! *Sanki-domain-oriented* view: access by [`Square`] (not by flat index), each
//! side's variant, the side to move, hands. Variants are validated and cached at
//! construction — so a `Position` always has well-formed Sanki styles.

pub mod board;
pub mod feen;
pub mod style;

use crate::domain::piece::Piece;
use crate::domain::side::Side;
use crate::domain::square::Square;
use crate::domain::variant::{Variant, VariantAssignment};
use sashite_epin::Identifier as Epin;
use sashite_qi::{Player, Qi};
use sashite_sin::Identifier as Sin;

/// A Sanki position: a [`Qi`] (EPIN pieces, SIN styles) augmented with the
/// variant assignment, validated at construction.
#[derive(Debug, Clone)]
pub struct Position {
    qi: Qi<Epin, Sin>,
    variants: VariantAssignment,
}

/// Error constructing a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionError {
    /// One of the position's styles is not a Sanki style (`W`/`C`/`J`).
    Style(style::StyleError),
}

impl Position {
    /// Builds a position from a `Qi`, validating the styles.
    ///
    /// # Errors
    /// [`PositionError::Style`] if either of the two styles is not a Sanki style.
    pub fn new(qi: Qi<Epin, Sin>) -> Result<Self, PositionError> {
        let variants = style::assignment(*qi.first_style(), *qi.second_style())
            .map_err(PositionError::Style)?;
        Ok(Self { qi, variants })
    }

    /// The underlying `Qi` (for FEEN encoding and `apply`'s transformations).
    #[inline]
    #[must_use]
    pub const fn qi(&self) -> &Qi<Epin, Sin> {
        &self.qi
    }

    /// Consumes the position and returns the `Qi`.
    #[inline]
    #[must_use]
    pub fn into_qi(self) -> Qi<Epin, Sin> {
        self.qi
    }

    /// The variant assignment for both sides.
    #[inline]
    #[must_use]
    pub const fn variants(&self) -> VariantAssignment {
        self.variants
    }

    /// The variant of side `side`.
    #[inline]
    #[must_use]
    pub const fn variant_of(&self, side: Side) -> Variant {
        self.variants.variant_of(side)
    }

    /// The side to move.
    #[inline]
    #[must_use]
    pub fn active_side(&self) -> Side {
        side_of_player(self.qi.turn())
    }

    /// The variant of the side to move.
    #[inline]
    #[must_use]
    pub fn active_variant(&self) -> Variant {
        self.variant_of(self.active_side())
    }

    /// The piece on square `square`, or `None` if the square is empty.
    #[inline]
    #[must_use]
    pub fn piece_at(&self, square: Square) -> Option<Piece> {
        self.qi
            .square(board::to_index(square))
            .copied()
            .map(Piece::new)
    }

    /// The pieces in side `side`'s hand, with their multiplicity.
    #[must_use]
    pub fn hand(&self, side: Side) -> Box<dyn Iterator<Item = (Piece, usize)> + '_> {
        match side {
            Side::First => Box::new(self.qi.first_hand().map(|(e, n)| (Piece::new(*e), n))),
            Side::Second => Box::new(self.qi.second_hand().map(|(e, n)| (Piece::new(*e), n))),
        }
    }

    /// The number of copies of `piece` in side `side`'s hand.
    #[inline]
    #[must_use]
    pub fn hand_count(&self, side: Side, piece: Piece) -> usize {
        let epin = piece.epin();
        match side {
            Side::First => self.qi.first_hand_count(&epin),
            Side::Second => self.qi.second_hand_count(&epin),
        }
    }
}

/// Mapping between `Qi`'s `Player` and our `Side`.
#[inline]
const fn side_of_player(player: Player) -> Side {
    match player {
        Player::First => Side::First,
        Player::Second => Side::Second,
    }
}

impl core::fmt::Display for PositionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Style(e) => write!(f, "invalid position style: {e}"),
        }
    }
}

impl core::error::Error for PositionError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Style(e) => Some(e),
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

    use super::board;
    use super::Position;
    use crate::domain::side::Side;
    use crate::domain::square::Square;
    use crate::domain::variant::Variant;
    use sashite_epin::Identifier as Epin;
    use sashite_qi::Qi;
    use sashite_sin::Identifier as Sin;

    fn sq(s: &str) -> Square {
        Square::parse(s).expect("valid square")
    }

    /// Builds a minimal chess vs ōgi position: a white king on e1.
    fn position_test() -> Position {
        let first = Sin::parse("W").expect("style W");
        let second = Sin::parse("j").expect("style j");
        let king = Epin::parse("K^").expect("king");
        let qi = Qi::new(&[8, 8], first, second)
            .expect("8x8 board")
            .board_diff([(board::to_index(sq("e1")), Some(king))])
            .expect("placement");
        Position::new(qi).expect("valid Sanki position")
    }

    #[test]
    fn variants_and_turn() {
        let p = position_test();
        assert_eq!(p.variant_of(Side::First), Variant::Chess);
        assert_eq!(p.variant_of(Side::Second), Variant::Ogi);
        // `Qi::new` leaves the first player to move.
        assert_eq!(p.active_side(), Side::First);
        assert_eq!(p.active_variant(), Variant::Chess);
    }

    #[test]
    fn piece_at() {
        let p = position_test();
        let king = p.piece_at(sq("e1")).expect("a king on e1");
        assert!(king.is_royal());
        assert_eq!(king.kind_letter(), 'K');
        assert_eq!(king.side(), Side::First);
        assert_eq!(p.piece_at(sq("e4")), None);
    }
}
