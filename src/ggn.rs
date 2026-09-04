//! GGN export — the **Geometry Gameplay Notation** (GGN 1.0.0) movement tables
//! of the three Sanki variants, generated from the same geometry the engine
//! moves by.
//!
//! A GGN document answers, for every piece of a variant (both sides, every
//! state the variant uses), every source (a square, or the hand `*`) and every
//! destination: *under which environmental pre-conditions is this movement
//! mechanically possible?* Each pre-condition set is a **possibility** — an
//! [LCN](https://sashite.dev/specs/lcn/1.0.0/) `must` / `deny` pair over named
//! squares — and a movement is possible iff at least one of its possibilities
//! is **satisfied** by the position.
//!
//! What the document carries is exactly the **pseudo-legal** layer of the rule
//! system (the manifest's `variants.<v>.ggn`, *Rule System — Sanki*): the
//! geometry of each piece, blocking, capture, the double step (keyed on the
//! `+` state), *en passant* (keyed on the victim's `-` state) and castling
//! (keyed on the rook's `+` state and the royal's non-check state). Everything
//! above it — royal safety, uchifuzume, promotion, the effects of a move, the
//! terminations — is the kernel's (*Kernel — Sanki*), and everything the
//! document says is checked against the engine by `tests/ggn_differential.rs`:
//! on positions reached by random play, the moves the engine admits are exactly
//! the satisfied possibilities that survive the kernel's filters, and the
//! engine's attack relation is exactly the GGN-derived one.
//!
//! One rule of the movement generator is deliberately **not** in the document:
//! **nifu** (no second unpromoted Fu of a side on a file). GGN evaluates a QPI
//! value by exact token equality, and a same-side Fu may stand on the file as
//! `F`, `+F` or `-F`; a `deny` object admits one value per square, so the three
//! cannot be denied together (GGN §7.3, *Limitation*). Nifu is therefore a
//! kernel drop restriction parameterised by the manifest (`drops.nifu`), like
//! uchifuzume — see the module test `nifu_is_not_in_the_document`.
//!
//! **QPI matching (normative for this export).** A QPI value in `must` / `deny`
//! matches a piece iff the piece's style letter (cased by its side), state
//! marker, letter and terminal marker are all equal to the value's — the
//! derivation marker `'` of EPIN, which QPI does not carry, is ignored.
//! `"empty"` is satisfied by an unoccupied square; `"enemy"` by a piece whose
//! side differs from the side of the QPI under which the possibility is listed
//! (GGN §7.2).

use crate::domain::piece::Piece;
use crate::domain::side::Side;
use crate::domain::square::Square;
use crate::domain::variant::{Variant, VariantAssignment};
use crate::movement::foot_soldier::crossed_river;
use crate::movement::{forward, gold_directions, leaper, slider, stepper};
use alloc_btree::{BTreeMap, Possibility};

/// Deterministic, sorted containers for the document (serde emits keys in
/// order, so two generations of the same variant are byte-identical).
mod alloc_btree {
    pub use std::collections::BTreeMap;

    /// One movement possibility: the LCN pre-conditions under which the
    /// movement is mechanically possible (GGN §6.3).
    #[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
    pub struct Possibility {
        /// All of these must hold (conjunction).
        #[serde(skip_serializing_if = "BTreeMap::is_empty")]
        pub must: BTreeMap<String, String>,
        /// None of these may hold (disjunction).
        #[serde(skip_serializing_if = "BTreeMap::is_empty")]
        pub deny: BTreeMap<String, String>,
    }

    /// QPI → source (`"*"` or a square) → destination (a square) → possibilities.
    pub type Document = BTreeMap<String, BTreeMap<String, BTreeMap<String, Vec<Possibility>>>>;
}

pub use alloc_btree::{Document as GgnDocument, Possibility as GgnPossibility};

/// The SIN style letter of a variant, uppercase (the first player's form; the
/// second player's is its lowercase).
#[inline]
#[must_use]
pub const fn style_letter(variant: Variant) -> char {
    match variant {
        Variant::Chess => 'W',
        Variant::Ogi => 'J',
        Variant::Xiongqi => 'C',
    }
}

/// The QPI token of a piece of `variant` (`W:+R`, `j:k^`, …), as the document
/// keys it and as `must` / `deny` values name it. The EPIN derivation marker is
/// not part of QPI and is dropped.
#[must_use]
pub fn qpi(piece: Piece, variant: Variant) -> String {
    let side = piece.side();
    let style = cased(style_letter(variant), side);
    let state = if piece.is_enhanced() {
        "+"
    } else if piece.is_diminished() {
        "-"
    } else {
        ""
    };
    let letter = cased(piece.kind_letter(), side);
    let terminal = if piece.is_royal() { "^" } else { "" };
    format!("{style}:{state}{letter}{terminal}")
}

#[inline]
fn cased(letter: char, side: Side) -> char {
    match side {
        Side::First => letter.to_ascii_uppercase(),
        Side::Second => letter.to_ascii_lowercase(),
    }
}

// ---------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------

/// A piece kind of a variant, with the states the variant lets it carry.
struct Kind {
    letter: char,
    royal: bool,
    states: &'static [State],
    droppable: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Normal,
    Enhanced,
    Diminished,
}

const N: &[State] = &[State::Normal];
const NED: &[State] = &[State::Normal, State::Enhanced, State::Diminished];
const ND: &[State] = &[State::Normal, State::Diminished];

const fn kind(letter: char, royal: bool, states: &'static [State], droppable: bool) -> Kind {
    Kind {
        letter,
        royal,
        states,
        droppable,
    }
}

/// The piece vocabulary of each variant: royals carry the check marker (`-`),
/// rook-class pieces the castling right (`+`/`-`), foot-soldiers the double-step
/// (`+`) and *en passant* (`-`) markers.
fn kinds(variant: Variant) -> Vec<Kind> {
    match variant {
        Variant::Chess => vec![
            kind('K', true, ND, false),
            kind('Q', false, N, false),
            kind('R', false, NED, false),
            kind('B', false, N, false),
            kind('N', false, N, false),
            kind('P', false, NED, false),
        ],
        Variant::Ogi => vec![
            kind('K', true, ND, false),
            kind('I', false, N, true),
            kind('R', false, NED, true),
            kind('B', false, N, true),
            kind('N', false, N, true),
            kind('F', false, NED, true),
            kind('T', false, N, false),
        ],
        Variant::Xiongqi => vec![
            kind('G', true, ND, false),
            kind('E', false, N, false),
            kind('R', false, NED, false),
            kind('B', false, N, false),
            kind('N', false, N, false),
            kind('S', false, NED, false),
        ],
    }
}

fn qpi_of(variant: Variant, side: Side, letter: char, royal: bool, state: State) -> String {
    let style = cased(style_letter(variant), side);
    let marker = match state {
        State::Normal => "",
        State::Enhanced => "+",
        State::Diminished => "-",
    };
    let terminal = if royal { "^" } else { "" };
    format!("{style}:{marker}{}{terminal}", cased(letter, side))
}

/// The complete GGN document of `variant`: every piece of the variant, both
/// sides, every state, every source, every destination.
#[must_use]
pub fn document(variant: Variant) -> GgnDocument {
    let mut doc = GgnDocument::new();
    for side in [Side::First, Side::Second] {
        for k in kinds(variant) {
            for &state in k.states {
                let key = qpi_of(variant, side, k.letter, k.royal, state);
                let mut sources: BTreeMap<String, BTreeMap<String, Vec<Possibility>>> =
                    BTreeMap::new();
                for from in Square::all() {
                    let dests = board_moves(variant, side, k.letter, state, from);
                    if !dests.is_empty() {
                        sources.insert(from.to_string(), dests);
                    }
                }
                if k.droppable && state == State::Normal {
                    let drops = drop_moves(side, k.letter);
                    if !drops.is_empty() {
                        sources.insert("*".to_owned(), drops);
                    }
                }
                if !sources.is_empty() {
                    doc.insert(key, sources);
                }
            }
        }
    }
    doc
}

/// The document as a `serde_json` value (keys sorted, no `$schema` member).
#[must_use]
pub fn to_json(doc: &GgnDocument) -> serde_json::Value {
    serde_json::to_value(doc).unwrap_or(serde_json::Value::Null)
}

type Dests = BTreeMap<String, Vec<Possibility>>;

fn push(dests: &mut Dests, to: Square, possibility: Possibility) {
    dests.entry(to.to_string()).or_default().push(possibility);
}

fn must(pairs: &[(Square, &str)]) -> Possibility {
    let mut p = Possibility::default();
    for (sq, value) in pairs {
        p.must.insert(sq.to_string(), (*value).to_owned());
    }
    p
}

/// The two possibilities of a destination reachable "if empty, or if enemy",
/// each prefixed by the same path conditions.
fn empty_or_enemy(dests: &mut Dests, path: &[Square], to: Square) {
    for value in ["empty", "enemy"] {
        let mut p = Possibility::default();
        for sq in path {
            p.must.insert(sq.to_string(), "empty".to_owned());
        }
        p.must.insert(to.to_string(), value.to_owned());
        push(dests, to, p);
    }
}

/// Walks the ray from `from` along `(df, dr)`, yielding each square with the
/// squares strictly between `from` and it.
fn ray(from: Square, df: i8, dr: i8) -> Vec<(Square, Vec<Square>)> {
    let mut out = Vec::new();
    let mut path = Vec::new();
    let mut cursor = from;
    while let Some(next) = cursor.offset(df, dr) {
        out.push((next, path.clone()));
        path.push(next);
        cursor = next;
    }
    out
}

fn slide(dests: &mut Dests, from: Square, dirs: &[(i8, i8)]) {
    for &(df, dr) in dirs {
        for (to, path) in ray(from, df, dr) {
            empty_or_enemy(dests, &path, to);
        }
    }
}

fn step(dests: &mut Dests, from: Square, dirs: &[(i8, i8)]) {
    for &(df, dr) in dirs {
        if let Some(to) = from.offset(df, dr) {
            empty_or_enemy(dests, &[], to);
        }
    }
}

fn board_moves(variant: Variant, side: Side, letter: char, state: State, from: Square) -> Dests {
    let mut dests = Dests::new();
    match letter {
        'K' | 'G' => {
            if letter == 'K' {
                step(&mut dests, from, &stepper::KING);
            } else {
                // General (xiongqi): an orthogonal step onto an empty square, a
                // capture at Chariot range.
                for &(df, dr) in &slider::ORTHOGONAL {
                    if let Some(to) = from.offset(df, dr) {
                        push(&mut dests, to, must(&[(to, "empty")]));
                    }
                    for (to, path) in ray(from, df, dr) {
                        let mut p = Possibility::default();
                        for sq in &path {
                            p.must.insert(sq.to_string(), "empty".to_owned());
                        }
                        p.must.insert(to.to_string(), "enemy".to_owned());
                        push(&mut dests, to, p);
                    }
                }
            }
            // Castling: only a royal not in check (state Normal), from its
            // origin square, toward a rook-class piece carrying `+`.
            if state == State::Normal {
                castling(&mut dests, variant, side, from);
            }
        }
        'Q' => slide(&mut dests, from, &slider::OCTILINEAR),
        'R' => slide(&mut dests, from, &slider::ORTHOGONAL),
        'B' => slide(&mut dests, from, &slider::DIAGONAL),
        'N' => step(&mut dests, from, &leaper::KNIGHT),
        'I' => {
            slide(&mut dests, from, &slider::DIAGONAL);
            step(&mut dests, from, &leaper::KNIGHT);
        }
        'E' => {
            slide(&mut dests, from, &slider::ORTHOGONAL);
            step(&mut dests, from, &leaper::KNIGHT);
        }
        'T' => step(&mut dests, from, &gold_directions(side)),
        'P' | 'F' | 'S' => foot_soldier(&mut dests, variant, side, state, from),
        _ => {}
    }
    dests
}

fn castling(dests: &mut Dests, variant: Variant, side: Side, from: Square) {
    let rank = match side {
        Side::First => 0,
        Side::Second => Square::RANK_COUNT.saturating_sub(1),
    };
    if from.file() != 4 || from.rank() != rank {
        return;
    }
    let rook = format!(
        "{}:+{}",
        cased(style_letter(variant), side),
        cased('R', side)
    );
    // Kingside: f, g empty; the rook on h. Queenside: b, c, d empty; the rook on a.
    let sides: [(u8, &[u8], u8); 2] = [(6, &[5, 6], 7), (2, &[1, 2, 3], 0)];
    for (to_file, between, rook_file) in sides {
        let (Some(to), Some(rook_sq)) = (Square::new(to_file, rank), Square::new(rook_file, rank))
        else {
            continue;
        };
        let mut p = Possibility::default();
        for &file in between {
            if let Some(sq) = Square::new(file, rank) {
                p.must.insert(sq.to_string(), "empty".to_owned());
            }
        }
        p.must.insert(rook_sq.to_string(), rook.clone());
        push(dests, to, p);
    }
}

/// The `-`-marked foot-soldiers an opponent of `side` may field, in the three
/// variants: the possible *en passant* victims.
fn victims(side: Side) -> [String; 3] {
    let opponent = side.flip();
    [
        format!("{}:-{}", cased('W', opponent), cased('P', opponent)),
        format!("{}:-{}", cased('J', opponent), cased('F', opponent)),
        format!("{}:-{}", cased('C', opponent), cased('S', opponent)),
    ]
}

fn foot_soldier(dests: &mut Dests, variant: Variant, side: Side, state: State, from: Square) {
    let fwd = forward(side);
    let Some(step1) = from.offset(0, fwd) else {
        return; // on the last rank: no forward move (a promoted piece has left)
    };
    // Forward step: the Pawn only moves there; the Fu and the Soldier also capture.
    push(dests, step1, must(&[(step1, "empty")]));
    if variant != Variant::Chess {
        push(dests, step1, must(&[(step1, "enemy")]));
    }
    // Double step, `+` state only: both squares empty.
    if state == State::Enhanced {
        if let Some(step2) = step1.offset(0, fwd) {
            push(dests, step2, must(&[(step1, "empty"), (step2, "empty")]));
        }
    }
    match variant {
        Variant::Chess => {
            // Diagonal capture, and *en passant* onto the skipped square: the
            // victim stands beside `from`, one rank behind the destination.
            for df in [-1, 1] {
                let Some(to) = from.offset(df, fwd) else {
                    continue;
                };
                push(dests, to, must(&[(to, "enemy")]));
                if let Some(victim_sq) = to.offset(0, forward(side.flip())) {
                    for victim in victims(side) {
                        let mut p = must(&[(to, "empty")]);
                        p.must.insert(victim_sq.to_string(), victim);
                        push(dests, to, p);
                    }
                }
            }
        }
        Variant::Ogi => {}
        Variant::Xiongqi => {
            // Past the river: a sideways step (move or capture), and *en
            // passant* sideways onto the skipped square.
            if crossed_river(side, from) {
                for df in [-1, 1] {
                    let Some(to) = from.offset(df, 0) else {
                        continue;
                    };
                    empty_or_enemy(dests, &[], to);
                    if let Some(victim_sq) = to.offset(0, forward(side.flip())) {
                        for victim in victims(side) {
                            let mut p = must(&[(to, "empty")]);
                            p.must.insert(victim_sq.to_string(), victim);
                            push(dests, to, p);
                        }
                    }
                }
            }
        }
    }
}

/// Drops (ōgi): any empty square — for the Fu, never its own last rank (nifu
/// is the kernel's, see the module documentation).
fn drop_moves(side: Side, letter: char) -> Dests {
    let last_rank = match side {
        Side::First => Square::RANK_COUNT.saturating_sub(1),
        Side::Second => 0,
    };
    let mut dests = Dests::new();
    for to in Square::all() {
        if letter == 'F' && to.rank() == last_rank {
            continue;
        }
        push(&mut dests, to, must(&[(to, "empty")]));
    }
    dests
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

/// Whether `possibility` is satisfied in a position, for a mover of side
/// `mover`. `piece_at` reads the board; `variants` gives each side's variant
/// (to build the QPI of the pieces met); `as_enemy`, when set, makes that
/// square count as **occupied by an enemy** whatever it holds — the projection
/// the attack relation is defined with (*Kernel — Sanki* §I.3, A1).
#[must_use]
pub fn satisfied(
    possibility: &GgnPossibility,
    mover: Side,
    variants: VariantAssignment,
    piece_at: impl Fn(Square) -> Option<Piece>,
    as_enemy: Option<Square>,
) -> bool {
    let holds = |key: &str, value: &str| -> bool {
        let Ok(sq) = Square::parse(key) else {
            return false;
        };
        if as_enemy == Some(sq) {
            return value == "enemy";
        }
        match (value, piece_at(sq)) {
            ("empty", occupant) => occupant.is_none(),
            ("enemy", Some(piece)) => !piece.belongs_to(mover),
            ("enemy", None) => false,
            (token, Some(piece)) => qpi(piece, variants.variant_of(piece.side())) == token,
            (_, None) => false,
        }
    };
    possibility.must.iter().all(|(k, v)| holds(k, v))
        && !possibility.deny.iter().any(|(k, v)| holds(k, v))
}

/// The destinations of the piece on `from` that some possibility of `doc`
/// makes possible in the position, for a mover of side `mover`.
#[must_use]
pub fn possible_destinations(
    doc: &GgnDocument,
    piece: Piece,
    variants: VariantAssignment,
    from: Square,
    piece_at: impl Fn(Square) -> Option<Piece>,
) -> Vec<Square> {
    let mover = piece.side();
    let key = qpi(piece, variants.variant_of(mover));
    let mut out = Vec::new();
    if let Some(dests) = doc
        .get(&key)
        .and_then(|sources| sources.get(&from.to_string()))
    {
        for (to, possibilities) in dests {
            let Ok(to) = Square::parse(to) else {
                continue;
            };
            if possibilities
                .iter()
                .any(|p| satisfied(p, mover, variants, &piece_at, None))
            {
                out.push(to);
            }
        }
    }
    out
}

/// The squares a held piece of `mover` may be dropped on, per `doc`.
#[must_use]
pub fn possible_drops(
    doc: &GgnDocument,
    piece: Piece,
    variants: VariantAssignment,
    piece_at: impl Fn(Square) -> Option<Piece>,
) -> Vec<Square> {
    let mover = piece.side();
    let key = qpi(piece, variants.variant_of(mover));
    let mut out = Vec::new();
    if let Some(dests) = doc.get(&key).and_then(|sources| sources.get("*")) {
        for (to, possibilities) in dests {
            let Ok(to) = Square::parse(to) else {
                continue;
            };
            if possibilities
                .iter()
                .any(|p| satisfied(p, mover, variants, &piece_at, None))
            {
                out.push(to);
            }
        }
    }
    out
}

/// Whether `target` is attacked by side `by` per `doc` (A1): some piece of `by`
/// has, from its square, a satisfied possibility to `target` whose destination
/// condition is `"enemy"`, `target` being taken as occupied by an enemy.
#[must_use]
pub fn attacked(
    doc: &GgnDocument,
    target: Square,
    by: Side,
    variants: VariantAssignment,
    piece_at: impl Fn(Square) -> Option<Piece>,
) -> bool {
    let target_key = target.to_string();
    Square::all().any(|from| {
        let Some(piece) = piece_at(from) else {
            return false;
        };
        if !piece.belongs_to(by) {
            return false;
        }
        let key = qpi(piece, variants.variant_of(by));
        doc.get(&key)
            .and_then(|sources| sources.get(&from.to_string()))
            .and_then(|dests| dests.get(&target_key))
            .is_some_and(|possibilities| {
                possibilities.iter().any(|p| {
                    p.must.get(&target_key).is_some_and(|v| v == "enemy")
                        && satisfied(p, by, variants, &piece_at, Some(target))
                })
            })
    })
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::*;

    #[test]
    fn documents_are_deterministic_and_non_empty() {
        for variant in [Variant::Chess, Variant::Ogi, Variant::Xiongqi] {
            let a = to_json(&document(variant)).to_string();
            let b = to_json(&document(variant)).to_string();
            assert_eq!(a, b);
            assert!(a.len() > 10_000);
        }
    }

    #[test]
    fn chess_pawn_double_step_needs_the_plus_state() {
        let doc = document(Variant::Chess);
        assert!(doc["W:+P"]["e2"].contains_key("e4"));
        assert!(!doc["W:P"]["e2"].contains_key("e4"));
        assert_eq!(doc["W:+P"]["e2"]["e4"][0].must["e3"], "empty");
    }

    #[test]
    fn castling_is_keyed_on_the_rook_state_and_absent_in_check() {
        let doc = document(Variant::Chess);
        let g1 = &doc["W:K^"]["e1"]["g1"];
        assert_eq!(g1.len(), 1);
        assert_eq!(g1[0].must["h1"], "W:+R");
        assert_eq!(g1[0].must["f1"], "empty");
        assert!(!doc["W:-K^"]["e1"].contains_key("g1"));
        let xq = document(Variant::Xiongqi);
        // The General also captures at Chariot range onto c8; the castling
        // possibility is the one naming the Chariot's state.
        assert!(xq["c:g^"]["e8"]["c8"]
            .iter()
            .any(|p| p.must.get("a8").is_some_and(|v| v == "c:+r")));
    }

    #[test]
    fn en_passant_names_the_three_victim_classes() {
        let doc = document(Variant::Chess);
        let f6 = &doc["W:P"]["e5"]["f6"];
        let victims: Vec<&String> = f6.iter().filter_map(|p| p.must.get("f5")).collect();
        assert_eq!(victims, vec!["w:-p", "j:-f", "c:-s"]);
    }

    #[test]
    fn nifu_is_not_in_the_document() {
        let doc = document(Variant::Ogi);
        let drops = &doc["J:F"]["*"];
        assert!(!drops.contains_key("e8"), "no Fu drop on the last rank");
        assert!(drops["e4"].iter().all(|p| p.deny.is_empty()));
    }
}
