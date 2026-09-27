//! Canonical PMN — the correspondence with the Ply content, checked against the
//! rules document (*SEI Rules Document — Sanki*, `sashite.sanki.kernel/1`).
//!
//! Two sources of moves:
//!
//! - the **legality corpus** (`tests/conformance/legality.json`): every admitted
//!   vector, with the canonical PMN pinned for the vectors the rules document
//!   cites;
//! - **random games** on the nine pairings, played through the engine.
//!
//! For every legal move, the test derives the PMN with [`pmn::to_pmn`], then
//! **asserts each rule of the document independently of the derivation**, from
//! the positions before and after alone — the operator from what moved and what
//! vanished, the actor suffix from the mover's identity, the capture suffix from
//! what entered the capturer's hand — reads the content back with
//! [`pmn::from_pmn`], accepts the string with [`pmn::parse_canonical`], and
//! checks that the other spellings of the same content are refused.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use sashite_sanki_engine::domain::half_move::Move;
use sashite_sanki_engine::domain::piece::Piece;
use sashite_sanki_engine::domain::side::Side;
use sashite_sanki_engine::domain::square::Square;
use sashite_sanki_engine::domain::variant::Variant;
use sashite_sanki_engine::pmn::{self, PmnError};
use sashite_sanki_engine::position::Position;
use sashite_sanki_engine::{engine, rules};
use std::collections::BTreeMap;

// ---- independent reading of the rules -------------------------------------

/// The token's letter in its side's case.
fn cased(piece: Piece) -> char {
    let letter = piece.epin().letter().as_char();
    match piece.side() {
        Side::First => letter.to_ascii_uppercase(),
        Side::Second => letter.to_ascii_lowercase(),
    }
}

/// Piece Identity without the transient state markers.
fn identity(piece: Piece) -> (char, bool, bool) {
    (cased(piece), piece.is_royal(), piece.epin().is_derived())
}

fn hand(position: &Position, side: Side) -> BTreeMap<String, usize> {
    position
        .hand(side)
        .map(|(piece, n)| (piece.to_string(), n))
        .collect()
}

/// What the rules document says the PMN of `mv` is, read from the two positions
/// alone (no use of the legality layer), together with the effect's name.
fn expected_pmn(before: &Position, mv: &Move, after: &Position) -> (String, &'static str) {
    let mover_side = before.active_side();
    let other_side = mover_side.flip();
    let mover_variant = before.variant_of(mover_side);
    let victim_variant = before.variant_of(other_side);
    let my_gain: Vec<(String, usize)> = {
        let (h0, h1) = (hand(before, mover_side), hand(after, mover_side));
        h1.iter()
            .filter_map(|(t, n)| {
                let gained = n.saturating_sub(h0.get(t).copied().unwrap_or(0));
                (gained > 0).then(|| (t.clone(), gained))
            })
            .collect()
    };
    assert_eq!(
        hand(before, other_side),
        hand(after, other_side),
        "the other hand never changes"
    );

    match mv {
        Move::Drop { piece: name, to } => {
            let dropped = after
                .piece_at(*to)
                .expect("the dropped piece stands on `to`");
            assert!(dropped.is_normal() && !dropped.is_royal(), "a drop is bare");
            assert_eq!(dropped.side(), mover_side);
            assert_eq!(mover_variant, Variant::Ogi, "only ōgi drops");
            assert_eq!(
                name.letter_for(Variant::Ogi),
                Some(cased(dropped).to_ascii_uppercase())
            );
            assert!(my_gain.is_empty());
            (format!("{dropped}*{to}"), "E5 drop")
        }
        Move::Board { from, to, actor } => {
            let mover = before.piece_at(*from).expect("a piece on `from`");
            let arrived = after.piece_at(*to).expect("a piece on `to`");
            assert_eq!(mover.side(), mover_side);
            let vanished: Vec<(Square, Piece)> = Square::all()
                .filter(|sq| sq != from)
                .filter_map(|sq| match (before.piece_at(sq), after.piece_at(sq)) {
                    (Some(piece), None) => Some((sq, piece)),
                    _ => None,
                })
                .collect();
            let own_moved: Vec<_> = vanished
                .iter()
                .filter(|(_, p)| p.side() == mover_side)
                .collect();
            let enemy_off_dst: Vec<_> = vanished
                .iter()
                .filter(|(sq, p)| p.side() == other_side && sq != to)
                .collect();
            let mut captured = before.piece_at(*to);
            let (operator, mut effect) = if !own_moved.is_empty() {
                assert!(own_moved.len() == 1 && captured.is_none() && enemy_off_dst.is_empty());
                assert!(mover.is_royal(), "only the royal castles");
                ('~', "E2 castling")
            } else if !enemy_off_dst.is_empty() {
                assert!(enemy_off_dst.len() == 1 && captured.is_none());
                captured = Some(enemy_off_dst[0].1);
                assert!(mover.is_foot_soldier() && captured.unwrap().is_foot_soldier());
                ('~', "E3 en passant")
            } else if captured.is_some() {
                ('+', "E1 capture")
            } else {
                ('-', "E1 quiet")
            };
            let mut out = format!("{from}{operator}{to}");
            // Actor suffix: iff the move promotes.
            let last_rank = match mover_side {
                Side::First => 7,
                Side::Second => 0,
            };
            let promotes =
                mover.is_foot_soldier() && to.rank() == last_rank && from.rank() != to.rank();
            if promotes {
                assert_ne!(identity(mover), identity(arrived));
                assert!(arrived.is_normal() && !arrived.is_royal());
                let choice = matches!(mover_variant, Variant::Chess | Variant::Xiongqi);
                match (choice, actor) {
                    (true, Some(name)) => assert_eq!(
                        name.letter_for(mover_variant),
                        Some(cased(arrived).to_ascii_uppercase())
                    ),
                    (false, None) => assert_eq!(cased(arrived).to_ascii_uppercase(), 'T'),
                    _ => panic!("actor named exactly when the variant lists several targets"),
                }
                out.push('=');
                out.push(cased(arrived));
                effect = if operator == '+' {
                    "E4 promotion (capture)"
                } else {
                    "E4 promotion"
                };
            } else {
                assert_eq!(
                    identity(mover),
                    identity(arrived),
                    "markers are recompositions"
                );
                assert!(actor.is_none());
            }
            // Capture suffix: iff the token entering the hand differs from the
            // victim's bare letter.
            match captured {
                Some(victim) => {
                    assert_eq!(my_gain.len(), 1, "one token enters the capturer's hand");
                    let (token, n) = &my_gain[0];
                    assert_eq!(*n, 1);
                    let expected_token =
                        expected_hand_token(victim, mover_variant, victim_variant, mover_side);
                    assert_eq!(*token, expected_token.to_string());
                    if *token != cased(victim).to_string() {
                        assert!(
                            mover_variant == Variant::Ogi
                                || cased(victim).eq_ignore_ascii_case(&'T'),
                            "an ōgi capturer always mutates; others only demote a tokin"
                        );
                        out.push('/');
                        out.push_str(token);
                    } else {
                        assert!(
                            mover_variant != Variant::Ogi
                                && !cased(victim).eq_ignore_ascii_case(&'T')
                        );
                    }
                }
                None => assert!(my_gain.is_empty()),
            }
            (out, effect)
        }
    }
}

/// The capture parameters of the three variants, as the rules document states
/// them (Kernel — Sanki §I.6, as the module applies them).
fn expected_hand_token(
    victim: Piece,
    capturer: Variant,
    victim_variant: Variant,
    capturer_side: Side,
) -> char {
    let mut base = cased(victim);
    if base.eq_ignore_ascii_case(&'T') {
        base = if base.is_ascii_uppercase() { 'F' } else { 'f' };
    }
    match capturer {
        Variant::Chess | Variant::Xiongqi => base,
        Variant::Ogi => {
            let fu = match capturer_side {
                Side::First => 'F',
                Side::Second => 'f',
            };
            if victim_variant != Variant::Ogi {
                fu
            } else {
                match capturer_side {
                    Side::First => base.to_ascii_uppercase(),
                    Side::Second => base.to_ascii_lowercase(),
                }
            }
        }
    }
}

/// Every check on one legal move; returns the effect's name for the tally.
fn check(before: &Position, mv: &Move) -> &'static str {
    let after = engine::apply(before, mv).expect("a legal move applies");
    let derived = pmn::to_pmn(before, mv).expect("a legal move has a canonical PMN");
    let (expected, effect) = expected_pmn(before, mv, &after);
    assert_eq!(derived, expected, "{mv:?} in {}", before.to_feen());
    assert_eq!(
        pmn::from_pmn(before, &derived).as_ref(),
        Ok(mv),
        "{derived}"
    );
    assert_eq!(
        pmn::parse_canonical(before, &derived).as_ref(),
        Ok(mv),
        "{derived}"
    );
    for other in other_spellings(&derived) {
        match pmn::parse_canonical(before, &other) {
            Ok(_) => panic!("{other} accepted beside {derived}"),
            Err(PmnError::NotCanonical { canonical }) => assert_eq!(canonical, derived),
            Err(_) => {}
        }
    }
    effect
}

/// Other well-formed spellings of the same content: the other operators, the
/// suffixes dropped or added.
fn other_spellings(canonical: &str) -> Vec<String> {
    let mut out = Vec::new();
    if canonical.contains('*') {
        return out;
    }
    for op in ['-', '+', '~'] {
        let spelled: String = canonical
            .chars()
            .map(|c| if matches!(c, '-' | '+' | '~') { op } else { c })
            .collect();
        if spelled != canonical {
            out.push(spelled);
        }
    }
    if let Some(i) = canonical.find(['=', '/']) {
        out.push(canonical[..i].to_owned());
    }
    if !canonical.contains('/') {
        out.push(format!("{canonical}/F"));
    }
    out
}

// ---- the legality corpus -----------------------------------------------------

#[test]
fn corpus_vectors_derive_and_round_trip() {
    let text = std::fs::read_to_string("tests/conformance/legality.json").expect("corpus");
    let json: serde_json::Value = serde_json::from_str(&text).expect("json");
    let pinned: BTreeMap<&str, &str> = [
        ("legality.quiet-move-rook-a1-a4", "a1-a4"),
        ("legality.kingside-castling-e1-g1", "e1~g1"),
        ("legality.ogi-kingside-castling-e1-g1", "e1~g1"),
        ("legality.xiongqi-kingside-castling-e1-g1", "e1~g1"),
        (
            "legality.en-passant-executed-the-taken-pawn-leaves-b4",
            "a4~b3",
        ),
        (
            "legality.promotion-pawn-a7-a8-to-queen-gives-check",
            "a7-a8=Q",
        ),
        ("legality.fu-drop-gi-no-double-step-prefix", "F*e4"),
        (
            "legality.insufficiency-ogi-capture-fills-the-hand",
            "e1+d1/F",
        ),
    ]
    .into_iter()
    .collect();
    let mut seen = 0;
    for vector in json["vectors"].as_array().expect("vectors") {
        if vector["legal"] != true {
            continue;
        }
        let before = Position::parse(vector["position"].as_str().unwrap()).unwrap();
        let mv = Move::parse(&vector["move"].to_string()).unwrap();
        check(&before, &mv);
        if let Some(expected) = pinned.get(vector["id"].as_str().unwrap()) {
            assert_eq!(pmn::to_pmn(&before, &mv).as_deref(), Ok(*expected));
            seen += 1;
        }
    }
    assert_eq!(seen, pinned.len(), "every pinned vector was found");
}

// ---- random games on the nine pairings --------------------------------------

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

#[test]
fn random_games_on_every_pairing() {
    let variants = [Variant::Chess, Variant::Ogi, Variant::Xiongqi];
    let games_per_pairing: usize = std::env::var("SANKI_PMN_GAMES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);
    let mut rng = Lcg(7);
    let mut tally: BTreeMap<&'static str, usize> = BTreeMap::new();
    for first in variants {
        for second in variants {
            for _ in 0..games_per_pairing {
                let mut position = rules::initial_position(first, second).expect("initial");
                for _ in 0..300 {
                    let moves = engine::legal_moves(&position);
                    if moves.is_empty() {
                        break;
                    }
                    // One move is played; on one position in eight, every legal
                    // move is checked, otherwise the played one.
                    let played = rng.below(moves.len());
                    if rng.below(8) == 0 {
                        for mv in &moves {
                            *tally.entry(check(&position, mv)).or_default() += 1;
                        }
                    } else {
                        *tally.entry(check(&position, &moves[played])).or_default() += 1;
                    }
                    position = engine::apply(&position, &moves[played]).expect("legal");
                    if !matches!(
                        engine::status(&position),
                        sashite_sanki_engine::domain::outcome::Verdict::Ongoing
                    ) {
                        break;
                    }
                }
            }
        }
    }
    eprintln!("moves checked by effect: {tally:?}");
    // En passant is rare in random play; the corpus covers it (four vectors,
    // the xiongqi lateral one included).
    for effect in [
        "E1 quiet",
        "E1 capture",
        "E2 castling",
        "E4 promotion",
        "E5 drop",
    ] {
        assert!(
            tally.get(effect).copied().unwrap_or(0) > 0,
            "{effect} never exercised: {tally:?}"
        );
    }
}
