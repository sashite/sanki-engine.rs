//! Clean public API for the Sanki rules engine.
//!
//! This module is the rules engine's façade: four ergonomic entry points that
//! compose the internal layers (`movement` -> `legality` -> `apply` ->
//! `canonicalize` -> `terminal`) without exposing their plumbing.
//!
//! - [`legal_moves`] — every legal move for the side to move;
//! - [`validate`] — is a given move legal here? (and why not, if illegal);
//! - [`apply`] — the canonical position resulting from a legal move;
//! - [`status`] — the position's intrinsic terminal status.
//!
//! These are pure functions over a [`Position`]; the per-session concerns
//! (clocks, the move history that repetition and the move-limit depend on) live
//! in [`crate::kernel`], and the adjudication layer in the companion
//! `sashite-sanki-arbiter` crate. In particular,
//! [`status`] reports only the **position-intrinsic** statuses — checkmate,
//! stalemate, `nomove`, dead position (insufficient material), or ongoing.
//! Repetition and the move-limit are history-dependent and therefore always
//! reported here as absent; a caller tracking history should use
//! [`crate::kernel`] for those.
//!
//! Since 0.4.0 the façade applies the **full rule system**, uchifuzume
//! included: [`validate`], [`apply`], and [`legal_moves`] reject/exclude a
//! mating Fu drop ([`IllegalReason::Uchifuzume`]), and [`status`] classifies
//! with the uchifuzume-aware legal set — exactly the legality the kernel (and
//! therefore the arbiter) enforces. The composition happens here because the
//! rule stacks the terminal layer (the opponent's legal-move search) on top of
//! `legality`, a dependency the `legality` module itself cannot take.

use crate::apply::{apply as apply_effect, Effect};
use crate::canonicalize::canonicalize;
use crate::domain::actor::ActorName;
use crate::domain::half_move::Move;
use crate::domain::outcome::{IllegalReason, Verdict};
use crate::domain::piece::Piece;
use crate::domain::side::Side;
use crate::domain::square::Square;
use crate::domain::variant::Variant;
use crate::legality::check::in_check;
use crate::legality::resolve::resolve;
use crate::movement::generate::pseudo_legal_destinations;
use crate::position::Position;
use crate::terminal::dead_position::is_dead_position;
use crate::terminal::legal_set::{has_full_legal_move, has_pseudo_legal_move};
use crate::terminal::uchifuzume::is_uchifuzume_drop;
use crate::terminal::{classify, TerminalConditions};

/// Last rank index of the first player (rank 8); a foot soldier reaching it
/// promotes. (`const` context: the subtraction is evaluated at compile time.)
const LAST_RANK: u8 = Square::RANK_COUNT - 1;

/// Resolves a move under the **full rule system**: the `legality` layer's
/// [`resolve`], plus the uchifuzume guard on a resolved drop. The single
/// composition point [`validate`], [`apply`], and [`legal_moves`] share — and
/// the same composition the kernel performs in its step.
fn resolve_full(position: &Position, mv: &Move) -> Result<Effect, IllegalReason> {
    let effect = resolve(position, mv)?;
    if let Effect::Drop { piece, to } = effect {
        if is_uchifuzume_drop(position, piece, to) {
            return Err(IllegalReason::Uchifuzume);
        }
    }
    Ok(effect)
}

/// Validates a move against `position`, under the full rule system
/// (uchifuzume included).
///
/// # Errors
/// Returns the [`IllegalReason`] that rejects the move; `Ok(())` if it is legal.
#[inline]
pub fn validate(position: &Position, mv: &Move) -> Result<(), IllegalReason> {
    resolve_full(position, mv).map(|_effect| ())
}

/// Applies a move to `position` and returns the canonical resulting position.
///
/// # Errors
/// Returns the [`IllegalReason`] if the move is illegal under the full rule
/// system (uchifuzume included). An internal failure of
/// the apply or canonicalize step denotes a broken invariant (unreachable on a
/// well-formed position) and is surfaced as [`IllegalReason::Malformed`],
/// consistent with [`crate::kernel`].
pub fn apply(position: &Position, mv: &Move) -> Result<Position, IllegalReason> {
    let effect: Effect = resolve_full(position, mv)?;
    let applied = apply_effect(position, effect).map_err(|_| IllegalReason::Malformed)?;
    canonicalize(&applied, &effect).map_err(|_| IllegalReason::Malformed)
}

/// The position's intrinsic terminal [`Verdict`] — checkmate, stalemate,
/// `nomove`, dead position (insufficient material), or [`Verdict::Ongoing`].
///
/// Repetition and the move-limit are history-dependent and are not evaluated
/// here (reported as absent); use [`crate::kernel`] when history is available.
#[must_use]
pub fn status(position: &Position) -> Verdict {
    let side = position.active_side();
    let variants = position.variants();
    let opponent_variant = position.variant_of(side.flip());
    let piece_at = |square: Square| position.piece_at(square);

    let first_hand: Vec<Piece> = position
        .hand(Side::First)
        .map(|(piece, _count)| piece)
        .collect();
    let second_hand: Vec<Piece> = position
        .hand(Side::Second)
        .map(|(piece, _count)| piece)
        .collect();
    // Both hands in one list: the predicates droppable-filter by side, and the
    // full reading's uchifuzume mate test needs the opponent's hand too.
    let hands: Vec<Piece> = first_hand
        .iter()
        .chain(second_hand.iter())
        .copied()
        .collect();

    classify(TerminalConditions {
        side_to_move: side,
        in_check: in_check(side, opponent_variant, piece_at),
        has_pseudo_legal_move: has_pseudo_legal_move(side, variants, piece_at, &hands),
        has_legal_move: has_full_legal_move(side, variants, piece_at, &hands),
        insufficient: is_dead_position(variants, piece_at, &first_hand, &second_hand),
        threefold_repetition: false,
        move_limit_reached: false,
    })
}

/// Every legal move available to the side to move in `position`, under the
/// full rule system (uchifuzume included).
///
/// Candidate moves are enumerated — pseudo-legal destinations of each own piece,
/// the king's castling targets, a foot soldier's neighbours (covering en
/// passant, whose landing square is always adjacent), and every hand drop in ōgi
/// — then filtered through [`validate`], so the result contains only fully legal
/// moves; in particular a mating Fu drop (uchifuzume) is excluded. Promotions
/// are expanded per target in chess and xiongqi (the actor
/// names the piece) and left implicit in ōgi (automatic).
#[must_use]
pub fn legal_moves(position: &Position) -> Vec<Move> {
    let side = position.active_side();
    let variant = position.active_variant();
    let mut moves = Vec::new();

    for from in Square::all() {
        let Some(piece) = position.piece_at(from) else {
            continue;
        };
        if !piece.belongs_to(side) {
            continue;
        }
        for to in candidate_destinations(position, from, piece) {
            if piece.is_foot_soldier() && on_last_rank(side, to) && variant != Variant::Ogi {
                for name in promotion_actors(variant) {
                    if let Ok(actor) = ActorName::parse(name) {
                        let mv = Move::Board {
                            from,
                            to,
                            actor: Some(actor),
                        };
                        if resolve_full(position, &mv).is_ok() {
                            moves.push(mv);
                        }
                    }
                }
            } else {
                let mv = Move::Board {
                    from,
                    to,
                    actor: None,
                };
                if resolve_full(position, &mv).is_ok() {
                    moves.push(mv);
                }
            }
        }
    }

    if variant == Variant::Ogi {
        for (piece, count) in position.hand(side) {
            if count == 0 {
                continue;
            }
            let Some(name) = ogi_drop_name(piece) else {
                continue;
            };
            let Ok(actor) = ActorName::parse(name) else {
                continue;
            };
            for to in Square::all() {
                if position.piece_at(to).is_some() {
                    continue;
                }
                let mv = Move::Drop {
                    piece: actor.clone(),
                    to,
                };
                if resolve_full(position, &mv).is_ok() {
                    moves.push(mv);
                }
            }
        }
    }

    moves
}

/// Candidate destination squares for the piece on `from`: its pseudo-legal
/// targets, plus the king's castling targets and a foot soldier's neighbours.
/// Deduplicated; [`validate`] is the final legality oracle.
fn candidate_destinations(position: &Position, from: Square, piece: Piece) -> Vec<Square> {
    let side = position.active_side();
    let variant = position.active_variant();
    let mut tos: Vec<Square> = Vec::new();

    for square in pseudo_legal_destinations(
        variant,
        side,
        piece.kind_letter(),
        piece.is_enhanced(),
        from,
        |square: Square| position.piece_at(square).map(Piece::side),
    ) {
        push_unique(square, &mut tos);
    }

    if variant == Variant::Chess && piece.kind_letter() == 'K' {
        for file in [2u8, 6u8] {
            if let Some(square) = Square::new(file, from.rank()) {
                push_unique(square, &mut tos);
            }
        }
    }

    if piece.is_foot_soldier() {
        for df in -1i8..=1 {
            for dr in -1i8..=1 {
                if df == 0 && dr == 0 {
                    continue;
                }
                if let Some(square) = from.offset(df, dr) {
                    push_unique(square, &mut tos);
                }
            }
        }
    }

    tos
}

/// Pushes `square` onto `tos` if not already present (small-set deduplication,
/// avoiding indexing arithmetic on a fixed array).
#[inline]
fn push_unique(square: Square, tos: &mut Vec<Square>) {
    if !tos.contains(&square) {
        tos.push(square);
    }
}

/// Whether `to` is on `side`'s promotion (last) rank.
#[inline]
const fn on_last_rank(side: Side, to: Square) -> bool {
    match side {
        Side::First => to.rank() == LAST_RANK,
        Side::Second => to.rank() == 0,
    }
}

/// The promotion-target piece-names of a variant (empty for ōgi, whose promotion
/// is automatic and carries no actor).
#[inline]
const fn promotion_actors(variant: Variant) -> &'static [&'static str] {
    match variant {
        Variant::Chess => &["queen", "rook", "bishop", "knight"],
        Variant::Xiongqi => &["chariot", "knight", "bear", "empress"],
        Variant::Ogi => &[],
    }
}

/// The ōgi drop piece-name for a held piece, or `None` if it is not an ōgi
/// droppable kind.
#[inline]
fn ogi_drop_name(piece: Piece) -> Option<&'static str> {
    match piece.kind_letter() {
        'F' => Some("fu"),
        'R' => Some("rook"),
        'B' => Some("bishop"),
        'N' => Some("knight"),
        'I' => Some("princess"),
        _ => None,
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

    use super::{apply, legal_moves, status, validate};
    use crate::domain::half_move::Move;
    use crate::domain::outcome::{IllegalReason, Verdict};
    use crate::domain::side::Side;
    use crate::domain::status::Status;
    use crate::position::Position;

    const CHESS_START: &str =
        "-rnbqk^bn-r/+p+p+p+p+p+p+p+p/8/8/8/8/+P+P+P+P+P+P+P+P/-RNBQK^BN-R / W/w";

    fn pos(feen: &str) -> Position {
        Position::parse(feen).expect("valid FEEN")
    }

    fn mv(content: &str) -> Move {
        Move::parse(content).expect("valid move")
    }

    #[test]
    fn legal_moves_initial_chess_position() {
        // FIDE reference: 20 legal moves from the starting position.
        assert_eq!(legal_moves(&pos(CHESS_START)).len(), 20);
    }

    #[test]
    fn validate_legal_and_illegal_move() {
        let position = pos("4k^3/8/8/8/8/8/8/R3K^3 / W/w");
        assert_eq!(validate(&position, &mv("[\"a1\",\"a4\",null]")), Ok(()));
        // No piece on h4: illegal.
        assert_eq!(
            validate(&position, &mv("[\"h4\",\"h5\",null]")),
            Err(IllegalReason::NoMoverPieceAtSource)
        );
    }

    #[test]
    fn apply_produces_the_canonical_position() {
        let position = pos("4k^3/8/8/8/8/8/8/R3K^3 / W/w");
        let next = apply(&position, &mv("[\"a1\",\"a4\",null]")).expect("legal move");
        assert_eq!(next.to_feen(), "4k^3/8/8/8/R7/8/8/4K^3 / w/W");
    }

    #[test]
    fn apply_rejects_an_illegal_move() {
        let position = pos("4k^3/8/8/8/8/8/8/4K^3 / W/w");
        assert_eq!(
            apply(&position, &mv("[\"a1\",\"a4\",null]")).unwrap_err(),
            IllegalReason::NoMoverPieceAtSource
        );
    }

    #[test]
    fn status_game_ongoing() {
        assert_eq!(status(&pos(CHESS_START)), Verdict::Ongoing);
    }

    #[test]
    fn status_checkmate() {
        // Back-rank mate: black King walled in on h8, white Rook on a8 along the rank.
        let mated = pos("R6-k^/6pp/8/8/8/8/8/4K^3 / w/W");
        assert_eq!(
            status(&mated),
            Verdict::decisive(Status::Checkmate, Side::Second)
        );
    }

    // Ōgi uchifuzume fixture (the kernel's): black King walled in on h8, white
    // Rook g1 and Knight f6, a white Fu in hand. Dropping the Fu on h7 would
    // deliver checkmate — illegal (uchifuzume); on h6 it is a quiet, legal drop.
    const UCHIFUZUME: &str = "7k^/8/5N2/8/8/8/8/4K^1R1 F/ J/j";

    #[test]
    fn validate_rejects_a_mating_fu_drop() {
        let position = pos(UCHIFUZUME);
        assert_eq!(
            validate(&position, &mv("[null,\"h7\",\"fu\"]")),
            Err(IllegalReason::Uchifuzume)
        );
        // A non-mating drop of the same Fu is legal.
        assert_eq!(validate(&position, &mv("[null,\"h6\",\"fu\"]")), Ok(()));
    }

    #[test]
    fn apply_rejects_a_mating_fu_drop() {
        let position = pos(UCHIFUZUME);
        assert_eq!(
            apply(&position, &mv("[null,\"h7\",\"fu\"]")).unwrap_err(),
            IllegalReason::Uchifuzume
        );
    }

    #[test]
    fn legal_moves_exclude_the_mating_fu_drop() {
        use crate::domain::half_move::Move as HalfMove;

        let position = pos(UCHIFUZUME);
        let moves = legal_moves(&position);
        let drops_to = |target: &str| {
            moves.iter().any(|m| {
                matches!(m, HalfMove::Drop { to, .. }
                    if *to == crate::domain::square::Square::parse(target).expect("square"))
            })
        };
        // The uchifuzume square is excluded; a harmless drop square is present.
        assert!(!drops_to("h7"));
        assert!(drops_to("h6"));
        // Every enumerated move validates under the full rule system.
        for m in &moves {
            assert_eq!(validate(&position, m), Ok(()));
        }
    }

    #[test]
    fn checking_fu_drop_stays_legal() {
        // Without the knight, the h7 drop is check but not mate: allowed.
        let position = pos("7k^/8/8/8/8/8/8/4K^1R1 F/ J/j");
        assert_eq!(validate(&position, &mv("[null,\"h7\",\"fu\"]")), Ok(()));
    }
}
