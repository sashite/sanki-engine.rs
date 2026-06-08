//! Foot-soldier promotion — resolving the transformed piece.
//!
//! The three variants share the **same zone**: promotion triggers when a
//! foot-soldier **ends its move on the last rank** (rank 8 for the first player,
//! rank 1 for the second), and is **mandatory** there. They differ in the choice:
//!
//! | Variant | Foot-soldier | Targets | Actor |
//! |---------|--------------|---------|-------|
//! | chess   | Pawn `P`     | `Q` `R` `B` `N` | **required** (names the target) |
//! | xiongqi | Soldier `S`  | `R` `N` `B` `E` | **required** |
//! | ōgi     | Fu `F`       | `T` (Tokin, unique) | **forbidden** (automatic) |
//!
//! A move that does not promote (another piece, or a foot-soldier not reaching
//! the last rank) must carry **no** actor. Drops never promote and belong to
//! [`crate::legality::drops`].
//!
//! This module decides the resulting **type**; cleaning up the `+`/`-` markers
//! belongs to [`crate::canonicalize`].

use crate::domain::actor::ActorName;
use crate::domain::piece::Piece;
use crate::domain::side::Side;
use crate::domain::square::Square;
use crate::domain::variant::Variant;
use core::fmt;

/// Rank index of the first player's last rank (rank 8).
const LAST_RANK: u8 = Square::RANK_COUNT - 1;

/// Resolves the promotion aspect of a board move.
///
/// Returns:
/// - `Ok(Some(piece))` — promotion: `piece` is the transformed piece to place;
/// - `Ok(None)` — no promotion; the caller keeps the moved piece;
/// - `Err(_)` — missing actor, superfluous actor, or invalid target (illegal move).
///
/// `mover` is the moving piece (side + type); `to` its destination square;
/// `actor` the move's actor field (the promotion target in chess/xiongqi).
pub fn resolve_promotion(
    variant: Variant,
    mover: Piece,
    to: Square,
    actor: Option<&ActorName>,
) -> Result<Option<Piece>, PromotionError> {
    let promotes = mover.is_foot_soldier() && on_promotion_rank(mover.side(), to);

    if !promotes {
        // Outside the zone (or non-promotable piece): no actor is expected.
        return match actor {
            Some(_) => Err(PromotionError::UnexpectedActor),
            None => Ok(None),
        };
    }

    match variant {
        // Automatic, unique promotion: the Fu becomes a Tokin, with no choice.
        Variant::Ogi => match actor {
            Some(_) => Err(PromotionError::UnexpectedActor),
            None => promote(mover, 'T'),
        },
        // Choice promotion: the actor names the target, validated by the table.
        Variant::Chess | Variant::Xiongqi => match actor {
            None => Err(PromotionError::ActorRequired),
            Some(name) => {
                let target = name
                    .letter_for(variant)
                    .ok_or(PromotionError::InvalidTarget)?;
                promote(mover, target)
            }
        },
    }
}

/// Builds the promoted piece, propagating a construction failure (invalid target
/// letter — unreachable for canonical targets, defensive branch).
#[inline]
fn promote(mover: Piece, target: char) -> Result<Option<Piece>, PromotionError> {
    mover
        .promoted_to(target)
        .map(Some)
        .ok_or(PromotionError::BuildFailed)
}

/// True if `to` is on side `side`'s last rank.
#[inline]
const fn on_promotion_rank(side: Side, to: Square) -> bool {
    match side {
        Side::First => to.rank() == LAST_RANK,
        Side::Second => to.rank() == 0,
    }
}

/// A promotion-resolution error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromotionError {
    /// The foot-soldier reaches the last rank but no actor names the target
    /// (chess/xiongqi, where the choice is required).
    ActorRequired,
    /// An actor is present where none is expected (automatic ōgi promotion, or a
    /// non-promoting move).
    UnexpectedActor,
    /// The actor names a target that is not a valid promotion for the variant.
    InvalidTarget,
    /// The promoted piece could not be built (invalid target letter).
    BuildFailed,
}

impl fmt::Display for PromotionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::ActorRequired => "promotion required: no target named",
            Self::UnexpectedActor => "superfluous actor: no choice promotion here",
            Self::InvalidTarget => "invalid promotion target for the variant",
            Self::BuildFailed => "cannot build the promoted piece",
        };
        f.write_str(message)
    }
}

impl core::error::Error for PromotionError {}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::{resolve_promotion, PromotionError};
    use crate::domain::actor::ActorName;
    use crate::domain::piece::Piece;
    use crate::domain::square::Square;
    use crate::domain::variant::Variant;
    use sashite_epin::Identifier as Epin;

    fn sq(s: &str) -> Square {
        Square::parse(s).expect("valid square")
    }

    fn piece(token: &str) -> Piece {
        Piece::new(Epin::parse(token).expect("valid EPIN"))
    }

    fn actor(name: &str) -> ActorName {
        ActorName::parse(name).expect("valid actor")
    }

    fn token_of(p: Option<Piece>) -> String {
        format!("{}", p.expect("promotion expected"))
    }

    #[test]
    fn chess_choice_promotion() {
        // White pawn reaching e8 -> Queen.
        let got = resolve_promotion(Variant::Chess, piece("P"), sq("e8"), Some(&actor("queen")))
            .expect("legal");
        assert_eq!(token_of(got), "Q");
        // Black pawn reaching e1 -> Knight (lowercase case).
        let got2 = resolve_promotion(Variant::Chess, piece("p"), sq("e1"), Some(&actor("knight")))
            .expect("legal");
        assert_eq!(token_of(got2), "n");
    }

    #[test]
    fn chess_without_actor_refused() {
        assert_eq!(
            resolve_promotion(Variant::Chess, piece("P"), sq("e8"), None),
            Err(PromotionError::ActorRequired)
        );
    }

    #[test]
    fn chess_invalid_target_refused() {
        // A Pawn cannot promote to King.
        assert_eq!(
            resolve_promotion(Variant::Chess, piece("P"), sq("e8"), Some(&actor("king"))),
            Err(PromotionError::InvalidTarget)
        );
    }

    #[test]
    fn xiongqi_promotion_empress() {
        let got = resolve_promotion(
            Variant::Xiongqi,
            piece("S"),
            sq("d8"),
            Some(&actor("empress")),
        )
        .expect("legal");
        assert_eq!(token_of(got), "E");
    }

    #[test]
    fn ogi_automatic_promotion() {
        // Fu on the last rank -> Tokin, with no actor.
        let got = resolve_promotion(Variant::Ogi, piece("F"), sq("c8"), None).expect("legal");
        assert_eq!(token_of(got), "T");
    }

    #[test]
    fn ogi_with_actor_refused() {
        // ōgi promotion is automatic: no actor is allowed.
        assert_eq!(
            resolve_promotion(Variant::Ogi, piece("F"), sq("c8"), Some(&actor("rook"))),
            Err(PromotionError::UnexpectedActor)
        );
    }

    #[test]
    fn outside_zone_no_promotion() {
        // Pawn not reaching the last rank: no promotion, no actor.
        assert_eq!(
            resolve_promotion(Variant::Chess, piece("P"), sq("e5"), None),
            Ok(None)
        );
    }

    #[test]
    fn outside_zone_with_actor_refused() {
        assert_eq!(
            resolve_promotion(Variant::Chess, piece("P"), sq("e5"), Some(&actor("queen"))),
            Err(PromotionError::UnexpectedActor)
        );
    }

    #[test]
    fn non_foot_soldier_piece_no_promotion() {
        // A Rook reaching the last rank does not promote.
        assert_eq!(
            resolve_promotion(Variant::Chess, piece("R"), sq("e8"), None),
            Ok(None)
        );
    }
}
