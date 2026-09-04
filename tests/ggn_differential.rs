//! GGN ↔ engine differential test (the inventory's spike 10.6, ADR-0033).
//!
//! On positions reached by random play from the nine variant pairings, checks:
//!
//! 1. **Completeness and soundness of the pseudo-legal layer.** The moves the
//!    engine admits (`engine::legal_moves`) are exactly the GGN-possible moves
//!    that survive the kernel's own filters (`engine::validate`): every legal
//!    move is GGN-possible, and every GGN-possible move the kernel rejects is
//!    rejected for a kernel reason (royal safety, uchifuzume, nifu, hand
//!    availability), never for a geometric one.
//! 2. **The attack relation (A1).** For every square and both sides, the
//!    engine's `is_attacked` equals the GGN-derived relation.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::collections::BTreeSet;

use sashite_sanki_engine::domain::actor::ActorName;
use sashite_sanki_engine::domain::half_move::Move;
use sashite_sanki_engine::domain::outcome::IllegalReason;
use sashite_sanki_engine::domain::piece::Piece;
use sashite_sanki_engine::domain::side::Side;
use sashite_sanki_engine::domain::square::Square;
use sashite_sanki_engine::domain::variant::Variant;
use sashite_sanki_engine::engine;
use sashite_sanki_engine::ggn::{self, GgnDocument};
use sashite_sanki_engine::movement::attack::is_attacked;
use sashite_sanki_engine::position::Position;

const GAMES_PER_PAIRING: u64 = 40;
const MAX_PLIES: usize = 200;

/// The two home ranks of a variant, as FEEN fragments (rank 8 first for the
/// second player, rank 1 last for the first player).
fn home(variant: Variant, side: Side) -> &'static str {
    match (variant, side) {
        (Variant::Chess, Side::Second) => "-rnbqk^bn-r/+p+p+p+p+p+p+p+p",
        (Variant::Chess, Side::First) => "+P+P+P+P+P+P+P+P/-RNBQK^BN-R",
        (Variant::Ogi, Side::Second) => "-rnbik^bn-r/+f+f+f+f+f+f+f+f",
        (Variant::Ogi, Side::First) => "+F+F+F+F+F+F+F+F/-RNBIK^BN-R",
        (Variant::Xiongqi, Side::Second) => "-rnbeg^bn-r/+s+s+s+s+s+s+s+s",
        (Variant::Xiongqi, Side::First) => "+S+S+S+S+S+S+S+S/-RNBEG^BN-R",
    }
}

fn start(first: Variant, second: Variant) -> Position {
    let feen = format!(
        "{}/8/8/8/8/{} / {}/{}",
        home(second, Side::Second),
        home(first, Side::First),
        ggn::style_letter(first),
        ggn::style_letter(second).to_ascii_lowercase()
    );
    Position::parse(&feen).expect("valid start position")
}

/// A tiny deterministic generator (no dev-dependency needed).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            items.get((self.next() as usize) % items.len())
        }
    }
}

/// A move projected onto what GGN describes: the geometry, not the actor.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Board(Square, Square),
    Drop(char, Square),
}

fn key(mv: &Move, variant: Variant) -> Key {
    match mv {
        Move::Board { from, to, .. } => Key::Board(*from, *to),
        Move::Drop { piece, to } => Key::Drop(piece.letter_for(variant).unwrap(), *to),
    }
}

fn promotion_actors(variant: Variant) -> &'static [&'static str] {
    match variant {
        Variant::Chess => &["queen", "rook", "bishop", "knight"],
        Variant::Xiongqi => &["chariot", "knight", "bear", "empress"],
        Variant::Ogi => &[],
    }
}

fn drop_name(letter: char) -> &'static str {
    match letter {
        'F' => "fu",
        'R' => "rook",
        'B' => "bishop",
        'N' => "knight",
        'I' => "princess",
        _ => "unknown",
    }
}

fn last_rank(side: Side) -> u8 {
    match side {
        Side::First => 7,
        Side::Second => 0,
    }
}

/// Every move the GGN document makes possible for the side to move, as
/// concrete `Move`s (promotions expanded per actor where the variant names one).
fn ggn_candidates(doc: &GgnDocument, position: &Position) -> Vec<Move> {
    let side = position.active_side();
    let variant = position.active_variant();
    let variants = position.variants();
    let piece_at = |sq: Square| position.piece_at(sq);
    let mut out = Vec::new();
    for from in Square::all() {
        let Some(piece) = piece_at(from) else {
            continue;
        };
        if !piece.belongs_to(side) {
            continue;
        }
        for to in ggn::possible_destinations(doc, piece, variants, from, piece_at) {
            let promotes = piece.is_foot_soldier()
                && to.rank() == last_rank(side)
                && !promotion_actors(variant).is_empty();
            if promotes {
                for name in promotion_actors(variant) {
                    out.push(Move::Board {
                        from,
                        to,
                        actor: Some(ActorName::parse(name).unwrap()),
                    });
                }
            } else {
                out.push(Move::Board {
                    from,
                    to,
                    actor: None,
                });
            }
        }
    }
    for (piece, count) in position.hand(side) {
        if count == 0 || !piece.belongs_to(side) {
            continue;
        }
        for to in ggn::possible_drops(doc, piece, variants, piece_at) {
            out.push(Move::Drop {
                piece: ActorName::parse(drop_name(piece.kind_letter())).unwrap(),
                to,
            });
        }
    }
    out
}

/// The reasons a GGN-possible move may be rejected: the kernel's filters above
/// the pseudo-legal layer. Anything else means the document over-generates.
fn kernel_reason(reason: IllegalReason) -> bool {
    matches!(
        reason,
        IllegalReason::LeavesRoyalInCheck
            | IllegalReason::Uchifuzume
            | IllegalReason::IllegalDrop
            | IllegalReason::IllegalPromotion
    )
}

static STATS: std::sync::Mutex<[u64; 8]> = std::sync::Mutex::new([0; 8]);

fn check_position(docs: &[(Variant, GgnDocument)], position: &Position, feen: &str) {
    let side = position.active_side();
    let variant = position.active_variant();
    let variants = position.variants();
    let doc = &docs.iter().find(|(v, _)| *v == variant).unwrap().1;

    // 1. Pseudo-legal layer.
    let legal_moves = engine::legal_moves(position);
    for m in &legal_moves {
        if let Move::Board { from, to, .. } = m {
            let piece = position.piece_at(*from).unwrap();
            let mut st = STATS.lock().unwrap();
            if piece.is_royal() && (i16::from(to.file()) - i16::from(from.file())).abs() == 2 {
                st[6] += 1;
            }
            if piece.is_foot_soldier()
                && to.file() != from.file()
                && position.piece_at(*to).is_none()
            {
                st[7] += 1;
            }
        }
    }
    let legal: BTreeSet<Key> = legal_moves.iter().map(|m| key(m, variant)).collect();
    let mut survivors: BTreeSet<Key> = BTreeSet::new();
    for mv in ggn_candidates(doc, position) {
        match engine::validate(position, &mv) {
            Ok(()) => {
                survivors.insert(key(&mv, variant));
            }
            Err(reason) => {
                let mut st = STATS.lock().unwrap();
                match reason {
                    IllegalReason::LeavesRoyalInCheck => st[1] += 1,
                    IllegalReason::Uchifuzume => st[2] += 1,
                    IllegalReason::IllegalDrop => st[3] += 1,
                    _ => st[4] += 1,
                };
                assert!(
                    kernel_reason(reason),
                    "GGN over-generates: {mv:?} rejected for {reason:?} in {feen}"
                )
            }
        }
        {
            let mut st = STATS.lock().unwrap();
            st[0] += legal.len() as u64;
        }
    }
    assert_eq!(
        survivors, legal,
        "GGN-possible ∩ kernel-admitted ≠ engine legal set in {feen}"
    );

    // 2. Attack relation, both sides, every square.
    let piece_at = |sq: Square| position.piece_at(sq);
    for by in [Side::First, Side::Second] {
        for target in Square::all() {
            let engine_says = is_attacked(target, by, variants.variant_of(by), piece_at);
            let ggn_says = ggn::attacked(
                doc_for(docs, variants.variant_of(by)),
                target,
                by,
                variants,
                piece_at,
            );
            if engine_says {
                STATS.lock().unwrap()[5] += 1;
            }
            assert_eq!(
                engine_says, ggn_says,
                "attack relation differs at {target} by {by:?} in {feen}"
            );
        }
    }
    let _ = side;
}

fn doc_for(docs: &[(Variant, GgnDocument)], variant: Variant) -> &GgnDocument {
    &docs.iter().find(|(v, _)| *v == variant).unwrap().1
}

#[test]
fn ggn_matches_the_engine_on_random_play() {
    let docs: Vec<(Variant, GgnDocument)> = [Variant::Chess, Variant::Ogi, Variant::Xiongqi]
        .into_iter()
        .map(|v| (v, ggn::document(v)))
        .collect();
    let all = [Variant::Chess, Variant::Ogi, Variant::Xiongqi];
    let mut rng = Lcg(0x5a5a_1234_9876_abcd);
    let mut positions = 0u64;
    for first in all {
        for second in all {
            for _ in 0..GAMES_PER_PAIRING {
                let mut position = start(first, second);
                for _ in 0..MAX_PLIES {
                    let feen = position.to_feen();
                    check_position(&docs, &position, &feen);
                    positions += 1;
                    if engine::status(&position).is_terminated() {
                        break;
                    }
                    let moves = engine::legal_moves(&position);
                    let Some(mv) = rng.pick(&moves) else {
                        break;
                    };
                    position = engine::apply(&position, mv).expect("legal by construction");
                }
            }
        }
    }
    assert!(
        positions > 1_000,
        "too few positions exercised: {positions}"
    );
    eprintln!("positions={positions} stats(cands,check,uchifuzume,drop,other,attacked,castlings,enpassants)={:?}", *STATS.lock().unwrap());
}

#[test]
fn every_kind_of_every_variant_has_entries() {
    for variant in [Variant::Chess, Variant::Ogi, Variant::Xiongqi] {
        let doc = ggn::document(variant);
        let expected: &[&str] = match variant {
            Variant::Chess => &[
                "W:K^", "W:-K^", "W:Q", "W:R", "W:+R", "W:-R", "W:B", "W:N", "W:P", "W:+P", "W:-P",
                "w:k^", "w:p",
            ],
            Variant::Ogi => &[
                "J:K^", "J:I", "J:R", "J:B", "J:N", "J:F", "J:+F", "J:-F", "J:T", "j:t", "j:f",
            ],
            Variant::Xiongqi => &[
                "C:G^", "C:-G^", "C:E", "C:R", "C:B", "C:N", "C:S", "C:+S", "C:-S", "c:g^", "c:s",
            ],
        };
        for key in expected {
            assert!(doc.contains_key(*key), "{variant:?} lacks {key}");
        }
        let _ = Piece::new;
    }
}
