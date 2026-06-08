//! Move → Effect resolution — the single entry point of the legality layer.
//!
//! This is the "Legality" stage of the kernel pipeline
//! (`legality -> apply -> canonicalize -> tick -> terminal`): given a parsed
//! [`Move`] and the current [`Position`], it classifies the half-move, dispatches
//! to the appropriate fine-grained resolver (castling, en passant, promotion,
//! drop, self-check), and yields either a fully resolved [`Effect`] ready for
//! [`crate::apply`] or the precise [`IllegalReason`] that rejects the move.
//!
//! The fine-grained modules of `legality` are the *tools*; this module is the
//! *composition* that turns them into the layer's verdict. It therefore names
//! [`apply::Effect`](crate::apply::Effect) — the shared hand-off vocabulary
//! between the adjacent `legality` and `apply` layers (apply already names
//! [`CastlingMove`](crate::legality::castling::CastlingMove), embedded in
//! `Effect`). Keeping a single `Effect` vocabulary, rather than mirroring it with
//! a legality-owned twin, gives one source of truth for the shape of board
//! effects.
//!
//! Scope boundaries:
//! - **Structural drop legality** is delegated to [`crate::legality::drops`];
//!   **uchifuzume** (drop-mate of a Fu) is a terminal-layer concern and is *not*
//!   evaluated here.
//! - **Transient markers** (`+`/`-`) are *not* recomposed here — that is the role
//!   of [`crate::canonicalize`]. `resolve` reports the mover's token as-is.

use crate::apply::Effect;
use crate::domain::actor::ActorName;
use crate::domain::half_move::Move;
use crate::domain::outcome::IllegalReason;
use crate::domain::piece::Piece;
use crate::domain::side::Side;
use crate::domain::square::Square;
use crate::domain::variant::Variant;
use crate::legality::castling::resolve_castling;
use crate::legality::drops::drop_is_legal;
use crate::legality::en_passant::en_passant_capture;
use crate::legality::promotion::resolve_promotion;
use crate::legality::self_check::move_is_safe;
use crate::movement::generate::pseudo_legal_destinations;
use crate::position::Position;

/// File of the king's home square (column `e`).
const KING_FILE: u8 = 4;
/// File of the king's kingside castling target (column `g`).
const KINGSIDE_FILE: u8 = 6;
/// File of the king's queenside castling target (column `c`).
const QUEENSIDE_FILE: u8 = 2;
/// Home rank of the first player (rank 1).
const HOME_RANK_FIRST: u8 = 0;
/// Home rank of the second player (rank 8).
const HOME_RANK_SECOND: u8 = Square::RANK_COUNT - 1;

/// Resolves a parsed half-move against `position` into the board-and-hands
/// [`Effect`] it produces, or the [`IllegalReason`] that rejects it.
///
/// The move is judged from the point of view of the side to move
/// ([`Position::active_side`]); a move by, or onto, the wrong side is reported
/// rather than silently reinterpreted.
pub fn resolve(position: &Position, mv: &Move) -> Result<Effect, IllegalReason> {
    match mv {
        Move::Board { from, to, actor } => resolve_board(position, *from, *to, actor.as_ref()),
        Move::Drop { piece, to } => resolve_drop(position, piece, *to),
    }
}

/// Resolves a board move (ordinary move, capture, castling, en passant or
/// promotion) into its [`Effect`].
fn resolve_board(
    position: &Position,
    from: Square,
    to: Square,
    actor: Option<&ActorName>,
) -> Result<Effect, IllegalReason> {
    let side = position.active_side();
    let variant = position.active_variant();
    let opponent_variant = position.variant_of(side.flip());
    let piece_at = |square: Square| position.piece_at(square);

    // The source square must carry a piece of the side to move.
    let mover = match piece_at(from) {
        Some(piece) if piece.belongs_to(side) => piece,
        _ => return Err(IllegalReason::NoMoverPieceAtSource),
    };

    // Castling (chess only): a king moving onto its c/g home square. Gating on
    // the variant avoids misreading an ōgi/xiongqi `+`-marked piece as a rook
    // bearing the castling right.
    if variant == Variant::Chess && mover.kind_letter() == 'K' && is_castle_target(side, from, to) {
        return match resolve_castling(side, opponent_variant, from, to, piece_at) {
            Some(castling) => Ok(Effect::Castle(castling)),
            None => Err(IllegalReason::IllegalCastling),
        };
    }

    // Geometric reachability (blocking and friendly-occupancy already excluded
    // by the generator). The closure is consumed by this single call.
    let reachable = pseudo_legal_destinations(
        variant,
        side,
        mover.kind_letter(),
        mover.is_enhanced(),
        from,
        |square: Square| piece_at(square).map(Piece::side),
    )
    .contains(&to);

    // En passant: a foot soldier reaching an *empty* square is never part of the
    // pseudo-legal set (which only offers diagonal/sideways captures onto an
    // occupied square), so it is resolved here as a fallback.
    let ep_captured = if !reachable && mover.is_foot_soldier() {
        en_passant_capture(variant, side, from, to, piece_at)
    } else {
        None
    };

    if !reachable && ep_captured.is_none() {
        // Refine the diagnostic for a destination occupied by a friendly piece;
        // any other unreachable destination is reported as illegal (a finer
        // BlockedPath verdict would require dedicated path analysis).
        return Err(match piece_at(to) {
            Some(piece) if piece.belongs_to(side) => IllegalReason::FriendlyDestination,
            _ => IllegalReason::IllegalDestination,
        });
    }

    // Promotion: resolve the piece that actually lands on `to`. Off the promotion
    // rank `resolve_promotion` returns `None` (the mover is placed unchanged) and
    // rejects a superfluous actor; en passant never lands on a promotion rank, so
    // the two cases never combine.
    let placed = match resolve_promotion(variant, mover, to, actor) {
        Ok(Some(promoted)) => promoted,
        Ok(None) => mover,
        Err(_) => return Err(IllegalReason::IllegalPromotion),
    };

    // Victim square: the en passant square if any, otherwise the destination when
    // it carries an enemy piece (a friendly destination was rejected above).
    let captured = match ep_captured {
        Some(square) => Some(square),
        None => piece_at(to).map(|_| to),
    };

    // The move must not leave the mover's own royal in check.
    if !move_is_safe(
        side,
        opponent_variant,
        Some(from),
        to,
        placed,
        ep_captured,
        piece_at,
    ) {
        return Err(IllegalReason::LeavesRoyalInCheck);
    }

    Ok(Effect::Board {
        from,
        to,
        placed,
        captured,
    })
}

/// Resolves a drop (placement from the reserve) into its [`Effect`].
fn resolve_drop(
    position: &Position,
    name: &ActorName,
    to: Square,
) -> Result<Effect, IllegalReason> {
    let side = position.active_side();
    let variant = position.active_variant();
    let opponent_variant = position.variant_of(side.flip());
    let piece_at = |square: Square| position.piece_at(square);

    // Resolve the named piece to its base letter within the active variant. A
    // name outside the variant's drop vocabulary makes the drop illegal.
    let letter = name.letter_for(variant).ok_or(IllegalReason::IllegalDrop)?;

    // Find a droppable held piece of that type. Held pieces of a given type are
    // interchangeable for dropping, so the first match is taken; reusing the
    // exact held token preserves its derivation flag in the resulting board
    // state. "Droppable" is the universal rule: the token's case matches its
    // holder's side.
    let held = position
        .hand(side)
        .find(|(piece, _)| piece.kind_letter() == letter && piece.belongs_to(side))
        .map(|(piece, _)| piece)
        .ok_or(IllegalReason::IllegalDrop)?;

    // Structural drop legality (empty destination; Fu last-rank and nifu).
    // Uchifuzume is evaluated by the terminal layer, not here.
    if drop_is_legal(held, to, piece_at).is_err() {
        return Err(IllegalReason::IllegalDrop);
    }

    // A drop must not leave the mover's own royal in check.
    if !move_is_safe(side, opponent_variant, None, to, held, None, piece_at) {
        return Err(IllegalReason::LeavesRoyalInCheck);
    }

    Ok(Effect::Drop { piece: held, to })
}

/// Whether `from -> to` is a castling attempt for `side`: the king stands on its
/// home `e` square and moves onto the `c` or `g` square of the same home rank.
#[inline]
const fn is_castle_target(side: Side, from: Square, to: Square) -> bool {
    let home = match side {
        Side::First => HOME_RANK_FIRST,
        Side::Second => HOME_RANK_SECOND,
    };
    from.rank() == home
        && from.file() == KING_FILE
        && to.rank() == home
        && (to.file() == KINGSIDE_FILE || to.file() == QUEENSIDE_FILE)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::resolve;
    use crate::apply::Effect;
    use crate::domain::half_move::Move;
    use crate::domain::outcome::IllegalReason;
    use crate::domain::piece::Piece;
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

    fn board_move(from: &str, to: &str) -> Move {
        Move::Board {
            from: sq(from),
            to: sq(to),
            actor: None,
        }
    }

    #[test]
    fn quiet_move_produces_a_board_without_capture() {
        let p = pos("4k^3/8/8/8/8/8/8/R3K^3 / W/w");
        let effect = resolve(&p, &board_move("a1", "a4")).expect("legal");
        assert_eq!(
            effect,
            Effect::Board {
                from: sq("a1"),
                to: sq("a4"),
                placed: piece("R"),
                captured: None,
            }
        );
    }

    #[test]
    fn ordinary_capture_carries_the_destination_square() {
        let p = pos("r3k^3/8/8/8/8/8/8/R3K^3 / W/w");
        let effect = resolve(&p, &board_move("a1", "a8")).expect("legal");
        assert_eq!(
            effect,
            Effect::Board {
                from: sq("a1"),
                to: sq("a8"),
                placed: piece("R"),
                captured: Some(sq("a8")),
            }
        );
    }

    #[test]
    fn no_piece_at_source() {
        let p = pos("4k^3/8/8/8/8/8/8/4K^3 / W/w");
        assert_eq!(
            resolve(&p, &board_move("a1", "a2")),
            Err(IllegalReason::NoMoverPieceAtSource)
        );
    }

    #[test]
    fn friendly_destination_diagnosed() {
        // Rook a1; friendly King on e1 blocks arrival along the rank.
        let p = pos("4k^3/8/8/8/8/8/8/R3K^3 / W/w");
        assert_eq!(
            resolve(&p, &board_move("a1", "e1")),
            Err(IllegalReason::FriendlyDestination)
        );
    }

    #[test]
    fn unreachable_destination() {
        // b3 is on no rook line from a1.
        let p = pos("4k^3/8/8/8/8/8/8/R3K^3 / W/w");
        assert_eq!(
            resolve(&p, &board_move("a1", "b3")),
            Err(IllegalReason::IllegalDestination)
        );
    }

    #[test]
    fn kingside_castling_produces_a_castle() {
        // King e1, Rook `+R` on h1; path clear and unattacked.
        let p = pos("4k^3/8/8/8/8/8/8/4K^2+R / W/w");
        let effect = resolve(&p, &board_move("e1", "g1")).expect("legal castling");
        assert_eq!(
            effect,
            Effect::Castle(CastlingMove {
                king_from: sq("e1"),
                king_to: sq("g1"),
                rook_from: sq("h1"),
                rook_to: sq("f1"),
            })
        );
    }

    #[test]
    fn castling_without_right_is_illegal() {
        // Same geometry but the Rook is `R` (without the `+` marker).
        let p = pos("4k^3/8/8/8/8/8/8/4K^2R / W/w");
        assert_eq!(
            resolve(&p, &board_move("e1", "g1")),
            Err(IllegalReason::IllegalCastling)
        );
    }

    #[test]
    fn en_passant_resolved() {
        // White pawn e5; diminished black pawn `-p` on d5. exd6 e.p.
        let p = pos("4k^3/8/8/3-pP3/8/8/8/4K^3 / W/w");
        let effect = resolve(&p, &board_move("e5", "d6")).expect("legal");
        assert_eq!(
            effect,
            Effect::Board {
                from: sq("e5"),
                to: sq("d6"),
                placed: piece("P"),
                captured: Some(sq("d5")),
            }
        );
    }

    #[test]
    fn chess_promotion_with_actor() {
        let p = pos("4k^3/1P6/8/8/8/8/8/4K^3 / W/w");
        let mv = Move::Board {
            from: sq("b7"),
            to: sq("b8"),
            actor: Some(crate::domain::actor::ActorName::parse("queen").expect("name")),
        };
        let effect = resolve(&p, &mv).expect("legal promotion");
        assert_eq!(
            effect,
            Effect::Board {
                from: sq("b7"),
                to: sq("b8"),
                placed: piece("Q"),
                captured: None,
            }
        );
    }

    #[test]
    fn chess_promotion_without_actor_is_illegal() {
        // In chess the promotion target must be named by the actor.
        let p = pos("4k^3/1P6/8/8/8/8/8/4K^3 / W/w");
        assert_eq!(
            resolve(&p, &board_move("b7", "b8")),
            Err(IllegalReason::IllegalPromotion)
        );
    }

    #[test]
    fn move_leaving_the_royal_in_check() {
        // White rook pinned on e2 in front of the King e1, black rook on e8: any
        // sideways move of the rook exposes the King.
        let p = pos("k^3r3/8/8/8/8/8/4R3/4K^3 / W/w");
        assert_eq!(
            resolve(&p, &board_move("e2", "a2")),
            Err(IllegalReason::LeavesRoyalInCheck)
        );
    }

    #[test]
    fn ogi_drop_legal() {
        // ōgi: the first player holds a Fu; drop on e5.
        let p = pos("4k^3/8/8/8/8/8/8/4K^3 F/ J/j");
        let mv = Move::Drop {
            piece: crate::domain::actor::ActorName::parse("fu").expect("name"),
            to: sq("e5"),
        };
        let effect = resolve(&p, &mv).expect("legal drop");
        assert_eq!(
            effect,
            Effect::Drop {
                piece: piece("F"),
                to: sq("e5"),
            }
        );
    }

    #[test]
    fn drop_piece_absent_from_hand() {
        // No Chariot (rook) in hand: illegal drop.
        let p = pos("4k^3/8/8/8/8/8/8/4K^3 F/ J/j");
        let mv = Move::Drop {
            piece: crate::domain::actor::ActorName::parse("rook").expect("name"),
            to: sq("e5"),
        };
        assert_eq!(resolve(&p, &mv), Err(IllegalReason::IllegalDrop));
    }

    #[test]
    fn drop_nifu_is_illegal() {
        // An unpromoted Fu on e3 forbids a second Fu on the e-file.
        let p = pos("4k^3/8/8/8/8/4F3/8/4K^3 F/ J/j");
        let mv = Move::Drop {
            piece: crate::domain::actor::ActorName::parse("fu").expect("name"),
            to: sq("e5"),
        };
        assert_eq!(resolve(&p, &mv), Err(IllegalReason::IllegalDrop));
    }
}
