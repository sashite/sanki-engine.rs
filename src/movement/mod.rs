//! Movement components and dispatch (variant, letter) → rule.
//!
//! The components produce the **pseudo-legal** targets (before the self-check
//! filter, which belongs to `legality`). They are *purely geometric*: they know
//! neither `Qi` nor pieces, only squares and an [`Occupancy`] view *relative to
//! the side that is moving*. The caller translates the concrete position into
//! this view.

pub mod attack;
pub mod foot_soldier;
pub mod generate;
pub mod leaper;
pub mod slider;
pub mod stepper;

use crate::domain::side::Side;

/// A square's occupancy, viewed **relative to the side that is moving**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Occupancy {
    /// Empty square.
    Empty,
    /// Square occupied by a **friendly** piece: it blocks, with no capture possible.
    Friendly,
    /// Square occupied by an **enemy** piece: capturable.
    Enemy,
}

impl Occupancy {
    /// Classifies a square from its occupant (`None` = empty), from side
    /// `mover`'s point of view.
    #[inline]
    #[must_use]
    pub fn of(occupant: Option<Side>, mover: Side) -> Self {
        match occupant {
            None => Self::Empty,
            Some(side) if side == mover => Self::Friendly,
            Some(_) => Self::Enemy,
        }
    }

    /// True if the square is occupied (friendly or enemy) — hence blocking for a
    /// slider, and opaque for attack detection.
    #[inline]
    #[must_use]
    pub const fn is_occupied(self) -> bool {
        matches!(self, Self::Friendly | Self::Enemy)
    }
}

/// The "forward" rank delta for side `side`: `first` goes up (`+1`), `second`
/// goes down (`-1`). Shared by the foot-soldier and the Tokin (Gold General).
#[inline]
#[must_use]
pub const fn forward(side: Side) -> i8 {
    match side {
        Side::First => 1,
        Side::Second => -1,
    }
}

/// The gold general's six directions (Tokin, `T`): the four orthogonals and the
/// two **forward** diagonals (never the backward diagonals). "Forward" depends on
/// the side. Single source shared by move generation and attack detection.
#[inline]
#[must_use]
pub const fn gold_directions(side: Side) -> [(i8, i8); 6] {
    let fwd = forward(side);
    [(1, 0), (-1, 0), (0, 1), (0, -1), (-1, fwd), (1, fwd)]
}
