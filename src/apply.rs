//! Applying the **visible mutations** of a move already judged legal: the move
//! itself, Rook relocation (castling), removing the victim (ordinary capture *or*
//! en passant), crediting the capture to hand, placing the promoted piece,
//! dropping from hand, and passing the turn to the opponent.
//!
//! Place in the kernel chain (`legality -> apply -> canonicalize -> …`):
//! `legality` has already **validated** the move and **resolved** its effects
//! (promoted piece, en-passant square, castling geometry); `apply` merely reports
//! them onto the [`Position`]. The **recomposition of transient markers** (`+`/`-`:
//! double step, en-passant vulnerability, castling right, check) is **not** done
//! here — it belongs to [`crate::canonicalize`]. `apply` therefore reports the
//! EPIN tokens *as-is*, markers included (a castling Rook stays `+R` until
//! canonicalization).
//!
//! This module is the only mutation-side layer coupled to [`Position`]: placing a
//! piece, emptying a square, crediting a hand and toggling the turn all go through
//! `Qi`'s immutable transformations. Transforming the captured piece into its
//! held form is delegated to [`crate::capture`], the single source of truth for
//! that rule.
//!
//! `apply` returns a [`Result`]: `Qi`'s transformations are fallible (off-board
//! index, hand underflow, cardinality), and a malformed [`Effect`] (an empty
//! "captured" square, an empty castling source) is reported rather than silently
//! ignored. On an effect coming from `legality`, none of these errors occurs;
//! they denote a broken **internal invariant** the kernel can handle without
//! panicking.

use crate::capture::capture_transform;
use crate::domain::piece::Piece;
use crate::domain::side::Side;
use crate::domain::square::Square;
use crate::legality::castling::CastlingMove;
use crate::position::board;
use crate::position::{Position, PositionError};
use sashite_epin::Identifier as Epin;
use sashite_qi::{Error as QiError, Qi};
use sashite_sin::Identifier as Sin;

/// The **board-and-hands** effect of a move already judged legal, independent of
/// the position: everything `apply` must report, except marker recomposition
/// (left to [`crate::canonicalize`]).
///
/// Built by the `legality`/resolution layer from a `Move` and the current
/// position; consumed as-is by [`apply`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// A board move: the piece leaves `from`, the piece `placed` (the mover, or
    /// its promotion) lands on `to`.
    ///
    /// `captured` carries the **victim**'s square, if any:
    /// - ordinary capture: `Some(to)` (the victim is on the destination square);
    /// - en passant: `Some(distinct_square)` (the victim is not on `to`);
    /// - quiet move: `None`.
    ///
    /// The victim, read from the current position, is credited — transformed by
    /// [`capture_transform`] — to the side to move's hand.
    Board {
        /// Origin square, emptied.
        from: Square,
        /// Destination square, where `placed` is set.
        to: Square,
        /// Piece set on `to` (the mover, promotion already resolved).
        placed: Piece,
        /// The victim's square, or `None` for a quiet move.
        captured: Option<Square>,
    },
    /// Castling (chess): joint King-and-Rook move, no capture.
    Castle(CastlingMove),
    /// Placement (drop, ōgi): `piece` leaves the side to move's hand and is set on
    /// `to` (an empty square, never a capture).
    Drop {
        /// Piece set, removed from the side to move's hand.
        piece: Piece,
        /// Destination square.
        to: Square,
    },
}

/// Failure applying an [`Effect`].
///
/// On an effect coming from `legality`, none of these cases occurs: they denote a
/// broken internal invariant, never a mere move illegality (which is filtered
/// upstream, outside this layer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyError {
    /// The effect names an empty "captured" square or an empty castling source.
    MalformedEffect,
    /// A `Qi` transformation failed (index, hand, cardinality).
    Qi(QiError),
    /// The resulting position does not have Sanki styles — unreachable, styles
    /// being invariant over a game.
    Position(PositionError),
}

/// Applies `effect` to `position` and returns the next position, with the **turn
/// toggled** to the opponent.
///
/// # Errors
/// - [`ApplyError::MalformedEffect`] if a square expected to be occupied is empty;
/// - [`ApplyError::Qi`] if a `Qi` transformation fails;
/// - [`ApplyError::Position`] if the resulting position is not Sanki.
pub fn apply(position: &Position, effect: Effect) -> Result<Position, ApplyError> {
    let mover = position.active_side();
    let qi = position.qi().clone();

    let qi = match effect {
        Effect::Board {
            from,
            to,
            placed,
            captured,
        } => apply_board(position, qi, mover, from, to, placed, captured)?,
        Effect::Castle(castling) => apply_castling(position, qi, castling)?,
        Effect::Drop { piece, to } => apply_drop(qi, mover, piece, to)?,
    };

    Position::new(qi.toggle()).map_err(ApplyError::Position)
}

/// Reports a board move: empties `from`, removes any victim, sets `placed` on
/// `to`, then credits the (transformed) capture to the side to move's hand.
fn apply_board(
    position: &Position,
    qi: Qi<Epin, Sin>,
    mover: Side,
    from: Square,
    to: Square,
    placed: Piece,
    captured: Option<Square>,
) -> Result<Qi<Epin, Sin>, ApplyError> {
    // Read the victim before any mutation (in an ordinary capture, `to` carries
    // it and would be overwritten by `placed`).
    let victim = match captured {
        Some(square) => Some(
            position
                .piece_at(square)
                .ok_or(ApplyError::MalformedEffect)?,
        ),
        None => None,
    };

    // Board changes: emptyings first, placement next, so that the placement wins
    // if an emptied square coincides with the destination (ordinary capture,
    // where `captured == to`).
    let mut changes: Vec<(usize, Option<Epin>)> = Vec::with_capacity(3);
    changes.push((board::to_index(from), None));
    if let Some(square) = captured {
        if square != to {
            // En passant: the victim occupies a square distinct from `to`.
            changes.push((board::to_index(square), None));
        }
    }
    changes.push((board::to_index(to), Some(placed.epin())));
    let qi = qi.board_diff(changes).map_err(ApplyError::Qi)?;

    // Credit the capture — transformed per the variants — to the side to move's
    // hand.
    match victim {
        Some(piece) => {
            let held = capture_transform(piece.epin(), position.variants());
            adjust_hand(qi, mover, held, 1).map_err(ApplyError::Qi)
        }
        None => Ok(qi),
    }
}

/// Reports a castling: joint King-and-Rook move, no capture. The tokens (King
/// `K^`, Rook `+R`) are reported as-is — consuming the castling right belongs to
/// [`crate::canonicalize`].
fn apply_castling(
    position: &Position,
    qi: Qi<Epin, Sin>,
    castling: CastlingMove,
) -> Result<Qi<Epin, Sin>, ApplyError> {
    let king = position
        .piece_at(castling.king_from)
        .ok_or(ApplyError::MalformedEffect)?;
    let rook = position
        .piece_at(castling.rook_from)
        .ok_or(ApplyError::MalformedEffect)?;

    // Emptyings first, placements next (the four squares of a FIDE castling are
    // distinct, but the order keeps placement winning on principle).
    let changes = [
        (board::to_index(castling.king_from), None),
        (board::to_index(castling.rook_from), None),
        (board::to_index(castling.king_to), Some(king.epin())),
        (board::to_index(castling.rook_to), Some(rook.epin())),
    ];
    qi.board_diff(changes).map_err(ApplyError::Qi)
}

/// Reports a drop: sets `piece` on `to` and removes one copy from the side to
/// move's hand.
fn apply_drop(
    qi: Qi<Epin, Sin>,
    mover: Side,
    piece: Piece,
    to: Square,
) -> Result<Qi<Epin, Sin>, ApplyError> {
    let qi = qi
        .board_diff([(board::to_index(to), Some(piece.epin()))])
        .map_err(ApplyError::Qi)?;
    adjust_hand(qi, mover, piece.epin(), -1).map_err(ApplyError::Qi)
}

/// Adjusts by `delta` copies the `token` in side `side`'s hand (positive: credit;
/// negative: removal).
#[inline]
fn adjust_hand(
    qi: Qi<Epin, Sin>,
    side: Side,
    token: Epin,
    delta: i32,
) -> Result<Qi<Epin, Sin>, QiError> {
    match side {
        Side::First => qi.first_hand_diff([(token, delta)]),
        Side::Second => qi.second_hand_diff([(token, delta)]),
    }
}

impl core::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MalformedEffect => {
                f.write_str("malformed effect: a square expected to be occupied is empty")
            }
            Self::Qi(e) => write!(f, "position transformation refused: {e}"),
            Self::Position(e) => write!(f, "resulting position is not Sanki: {e}"),
        }
    }
}

impl core::error::Error for ApplyError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::MalformedEffect => None,
            Self::Qi(e) => Some(e),
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

    use super::{apply, Effect};
    use crate::domain::piece::Piece;
    use crate::domain::side::Side;
    use crate::domain::square::Square;
    use crate::legality::castling::CastlingMove;
    use crate::position::Position;
    use sashite_epin::Identifier as Epin;

    fn pos(feen: &str) -> Position {
        Position::parse(feen).expect("valid Sanki FEEN")
    }

    fn sq(s: &str) -> Square {
        Square::parse(s).expect("valid square")
    }

    fn piece(token: &str) -> Piece {
        Piece::new(Epin::parse(token).expect("valid EPIN token"))
    }

    fn token_at(position: &Position, square: &str) -> Option<String> {
        position.piece_at(sq(square)).map(|p| p.to_string())
    }

    #[test]
    fn quiet_move_relocates_and_toggles_turn() {
        // White rook a1, white King e1, black King e8. Ra1-a4.
        let before = pos("4k^3/8/8/8/8/8/8/R3K^3 / W/w");
        let after = apply(
            &before,
            Effect::Board {
                from: sq("a1"),
                to: sq("a4"),
                placed: piece("R"),
                captured: None,
            },
        )
        .expect("legal move");

        assert_eq!(token_at(&after, "a1"), None);
        assert_eq!(token_at(&after, "a4"), Some("R".to_owned()));
        assert_eq!(after.active_side(), Side::Second, "the turn toggles");
    }

    #[test]
    fn ordinary_capture_credits_to_hand() {
        // chess vs chess: the capturer's hand is inert (opponent's case kept), but
        // the piece does enter it (Rxa8).
        let before = pos("r3k^3/8/8/8/8/8/8/R3K^3 / W/w");
        let after = apply(
            &before,
            Effect::Board {
                from: sq("a1"),
                to: sq("a8"),
                placed: piece("R"),
                captured: Some(sq("a8")),
            },
        )
        .expect("legal move");

        assert_eq!(
            token_at(&after, "a8"),
            Some("R".to_owned()),
            "white rook on a8"
        );
        assert_eq!(token_at(&after, "a1"), None);
        // The captured black rook (`r`) enters, inert, into the first player's hand.
        assert_eq!(after.hand_count(Side::First, piece("r")), 1);
        assert_eq!(after.hand_count(Side::Second, piece("r")), 0);
    }

    #[test]
    fn en_passant_removes_victim_off_destination() {
        // White pawn e5; black pawn d5 diminished (`-p`, double step played). exd6 e.p.
        let before = pos("4k^3/8/8/3-pP3/8/8/8/4K^3 / W/w");
        let after = apply(
            &before,
            Effect::Board {
                from: sq("e5"),
                to: sq("d6"),
                placed: piece("P"),
                captured: Some(sq("d5")),
            },
        )
        .expect("legal move");

        assert_eq!(
            token_at(&after, "d6"),
            Some("P".to_owned()),
            "pawn arrived on d6"
        );
        assert_eq!(token_at(&after, "e5"), None, "origin square emptied");
        assert_eq!(token_at(&after, "d5"), None, "victim removed (off d6)");
        // The victim's base form (`-p` -> `p`) in the first player's hand (inert).
        assert_eq!(after.hand_count(Side::First, piece("p")), 1);
    }

    #[test]
    fn promotion_places_the_promoted_piece() {
        // White pawn b7; promotion to Queen on b8 (target already resolved by
        // `legality`, passed via `placed`).
        let before = pos("4k^3/1P6/8/8/8/8/8/4K^3 / W/w");
        let after = apply(
            &before,
            Effect::Board {
                from: sq("b7"),
                to: sq("b8"),
                placed: piece("Q"),
                captured: None,
            },
        )
        .expect("legal move");

        assert_eq!(
            token_at(&after, "b8"),
            Some("Q".to_owned()),
            "promoted Queen"
        );
        assert_eq!(token_at(&after, "b7"), None);
    }

    #[test]
    fn kingside_castling_moves_king_and_rook() {
        // White King e1, Rook `+R` on h1. The tokens are reported as-is: the Rook
        // stays `+R` (removing the right belongs to canonicalization).
        let before = pos("4k^3/8/8/8/8/8/8/4K^2+R / W/w");
        let after = apply(
            &before,
            Effect::Castle(CastlingMove {
                king_from: sq("e1"),
                king_to: sq("g1"),
                rook_from: sq("h1"),
                rook_to: sq("f1"),
            }),
        )
        .expect("legal castling");

        assert_eq!(token_at(&after, "g1"), Some("K^".to_owned()), "King on g1");
        assert_eq!(
            token_at(&after, "f1"),
            Some("+R".to_owned()),
            "Rook on f1, marker kept"
        );
        assert_eq!(token_at(&after, "e1"), None);
        assert_eq!(token_at(&after, "h1"), None);
        assert_eq!(after.active_side(), Side::Second);
    }

    #[test]
    fn drop_places_piece_and_removes_from_hand() {
        // ōgi vs ōgi, the first player holds a Fu in hand: drop on e5.
        let before = pos("4k^3/8/8/8/8/8/8/4K^3 F/ J/j");
        assert_eq!(
            before.hand_count(Side::First, piece("F")),
            1,
            "Fu in hand initially"
        );

        let after = apply(
            &before,
            Effect::Drop {
                piece: piece("F"),
                to: sq("e5"),
            },
        )
        .expect("legal drop");

        assert_eq!(
            token_at(&after, "e5"),
            Some("F".to_owned()),
            "Fu placed on e5"
        );
        assert_eq!(
            after.hand_count(Side::First, piece("F")),
            0,
            "Fu removed from hand"
        );
        assert_eq!(after.active_side(), Side::Second);
    }

    #[test]
    fn ogi_capture_flips_case_in_hand() {
        // ōgi (first) captures an enemy ōgi piece: the capture flips to the
        // capturer's side and becomes droppable (Rxd8: `r` -> `R`).
        let before = pos("3r3k^/8/8/8/8/8/8/3R3K^ / J/j");
        let after = apply(
            &before,
            Effect::Board {
                from: sq("d1"),
                to: sq("d8"),
                placed: piece("R"),
                captured: Some(sq("d8")),
            },
        )
        .expect("legal move");

        assert_eq!(token_at(&after, "d8"), Some("R".to_owned()));
        // The black rook `r` becomes `R` (capturer's side), droppable.
        assert_eq!(
            after.hand_count(Side::First, piece("R")),
            1,
            "capture credited to hand"
        );
    }

    #[test]
    fn malformed_effect_empty_captured_square() {
        // `captured` names an empty square: incoherent effect, reported without panicking.
        let before = pos("4k^3/8/8/8/8/8/8/R3K^3 / W/w");
        let result = apply(
            &before,
            Effect::Board {
                from: sq("a1"),
                to: sq("a4"),
                placed: piece("R"),
                captured: Some(sq("h5")), // h5 is empty
            },
        );
        assert!(matches!(result, Err(super::ApplyError::MalformedEffect)));
    }
}
