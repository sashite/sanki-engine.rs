//! Conformance-vector generator (ADR-0010) — emits corpus **additions** for
//! `web-specs.md/nostr/conformance/`.
//!
//! Provenance rule (conformance README): expected values are never hand-written —
//! every vector is **computed and verified by the engine itself** before being
//! emitted. Inputs (positions, move sequences) are curated here; each carries the
//! designer's *intent* (expected legality / status / terminal ply), and the
//! generator FAILS if the engine disagrees, so a drift between intent and rules
//! can never silently reach the corpus.
//!
//! Output (deterministic byte-for-byte across runs — fixed tables, no clocks):
//! - `legality-additions.json` — category A vectors (2-space style, as the
//!   existing `legality.json`);
//! - `scenarios-additions.json` — category D vectors carrying the ADR-0010
//!   `expectedTermination` field (tab style, as the existing `scenarios.json`).
//!
//! Usage: `cargo run --example gen_vectors [-- <output-dir>]` (default
//! `corpus-additions/`). Integration into the corpus files (merge, version bump,
//! README) is PR-3's concern; this tool only produces verified candidates.
//!
//! Replay verification uses zero elapsed time (like `tests/conformance.rs`), so
//! verdicts reflect the rule system alone; the emitted `timedAt` metadata only
//! feeds the selection layer of the downstream harnesses.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use sashite_sanki_engine::domain::half_move::Move;
use sashite_sanki_engine::domain::outcome::Verdict;
use sashite_sanki_engine::domain::time::{Duration, Timestamp};
use sashite_sanki_engine::domain::time_control::{Period, TimeControl};
use sashite_sanki_engine::kernel::state::SessionState;
use sashite_sanki_engine::kernel::step::step;
use sashite_sanki_engine::position::Position;

// ---------------------------------------------------------------------------
// Moves
// ---------------------------------------------------------------------------

/// A `[src, dst, actor]` half-move (src `None` = drop; actor a piece name).
#[derive(Clone)]
struct MoveSpec {
    src: Option<String>,
    dst: String,
    actor: Option<String>,
}

fn mv(src: &str, dst: &str) -> MoveSpec {
    MoveSpec {
        src: Some(src.to_owned()),
        dst: dst.to_owned(),
        actor: None,
    }
}

fn drop_mv(dst: &str, actor: &str) -> MoveSpec {
    MoveSpec {
        src: None,
        dst: dst.to_owned(),
        actor: Some(actor.to_owned()),
    }
}

/// The Ply `content` JSON of a move.
fn content_of(half_move: &MoveSpec) -> String {
    let src = match &half_move.src {
        Some(s) => format!("\"{s}\""),
        None => "null".to_owned(),
    };
    let actor = match &half_move.actor {
        Some(a) => format!("\"{a}\""),
        None => "null".to_owned(),
    };
    format!("[{src},\"{}\",{actor}]", half_move.dst)
}

// ---------------------------------------------------------------------------
// Engine driving (mirrors tests/conformance.rs)
// ---------------------------------------------------------------------------

/// A time control generous enough never to flag at zero elapsed time.
fn neutral_time_control() -> TimeControl {
    let period = Period::new(Duration::from_secs(3_600), None, None).expect("valid period");
    TimeControl::new(period, Vec::new())
}

/// The corpus status string of a verdict: `ongoing`, or the terminal status.
fn verdict_status(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Ongoing => "ongoing".to_owned(),
        Verdict::Terminated { status, .. } => status.to_string(),
    }
}

struct Evaluation {
    legal: bool,
    status: String,
    result: String,
}

/// One half-move driven through the full rule system (`kernel::step`) from a
/// fresh position — the exact path `tests/conformance.rs` replays.
fn evaluate(feen: &str, half_move: &MoveSpec) -> Result<Evaluation, String> {
    let position =
        Position::parse(feen).map_err(|error| format!("invalid FEEN {feen}: {error:?}"))?;
    let content = content_of(half_move);
    let parsed = Move::parse(&content)
        .map_err(|error| format!("move did not parse {content}: {error:?}"))?;
    let state = SessionState::start(position, neutral_time_control(), Timestamp::from_unix(0));
    let outcome = step(state, &parsed, Timestamp::from_unix(0)).outcome;
    let status = verdict_status(&outcome.verdict);
    Ok(Evaluation {
        legal: status != "illegalmove",
        status,
        result: outcome.position,
    })
}

// ---------------------------------------------------------------------------
// Category A — curated legality/status cases
// ---------------------------------------------------------------------------

struct LegalityCase {
    id: &'static str,
    note: &'static str,
    position: &'static str,
    half_move: MoveSpec,
    /// Designer's intent — generation fails if the engine disagrees.
    expect_legal: bool,
    expect_status: Option<&'static str>,
}

fn legality_cases() -> Vec<LegalityCase> {
    vec![
        LegalityCase {
            id: "legality.insufficiency-chess-last-capture",
            note: "the capture that leaves the two lone Kings draws immediately (insufficient); the captured rook sits inert in the hand",
            position: "4k^3/8/8/8/8/8/8/3rK^3 / W/w",
            half_move: mv("e1", "d1"),
            expect_legal: true,
            expect_status: Some("insufficient"),
        },
        LegalityCase {
            id: "legality.insufficiency-xiongqi-last-capture",
            note: "the General captures like a Chariot; taking the last piece leaves the two lone Generals — insufficient",
            position: "4g^3/8/8/8/8/8/8/3rG^3 / C/c",
            half_move: mv("e1", "d1"),
            expect_legal: true,
            expect_status: Some("insufficient"),
        },
        LegalityCase {
            id: "legality.insufficiency-mixed-king-vs-general",
            note: "cross-variant: a lone King versus a lone General is declared drawn by insufficient material outright (deciders' ruling, 2026-07-10 — not strictly a dead position, but no side can force a win)",
            position: "4g^3/8/8/8/8/8/8/3rK^3 / W/c",
            half_move: mv("e1", "d1"),
            expect_legal: true,
            expect_status: Some("insufficient"),
        },
        LegalityCase {
            id: "legality.insufficiency-ogi-capture-fills-the-hand",
            note: "ōgi: capturing the last piece converts it into the hand — droppable material remains, so lone-Kings-on-the-board is NOT insufficient here",
            position: "4k^3/8/8/8/8/8/8/3fK^3 / J/j",
            half_move: mv("e1", "d1"),
            expect_legal: true,
            expect_status: Some("ongoing"),
        },
        LegalityCase {
            id: "legality.deadposition-ogi-lone-kings-live",
            note: "pure ōgi performs NO dead-position detection (rules-of-ogi §Dead-Position Detection): even the materially dead lone-Kings-with-empty-hands configuration (unreachable from the standard start) does not end the game",
            position: "4k^3/8/8/8/8/8/8/4K^3 / J/j",
            half_move: mv("e1", "d1"),
            expect_legal: true,
            expect_status: Some("ongoing"),
        },
        LegalityCase {
            id: "legality.deadposition-chess-kb-vs-k",
            note: "chess: the capture that leaves King + Bishop versus King is an immediate dead-position draw (rules-of-chess §Dead-Position Detection)",
            position: "4k^3/8/8/8/8/8/8/3rK^2B / W/w",
            half_move: mv("e1", "d1"),
            expect_legal: true,
            expect_status: Some("insufficient"),
        },
        LegalityCase {
            id: "legality.deadposition-chess-kn-vs-k",
            note: "chess: the capture that leaves King + Knight versus King is an immediate dead-position draw",
            position: "4k^3/8/8/8/8/8/8/3rK^2N / W/w",
            half_move: mv("e1", "d1"),
            expect_legal: true,
            expect_status: Some("insufficient"),
        },
        LegalityCase {
            id: "legality.deadposition-chess-same-colour-bishops",
            note: "chess: Kings and Bishops only, all Bishops on the same colour (B h1, b c8) — dead as soon as the last other piece is captured",
            position: "2b1k^3/8/8/8/8/8/8/3rK^2B / W/w",
            half_move: mv("e1", "d1"),
            expect_legal: true,
            expect_status: Some("insufficient"),
        },
        LegalityCase {
            id: "legality.deadposition-chess-opposite-bishops-live",
            note: "chess: Bishops on OPPOSITE colours (B h1, b b8) are not a dead position — a cooperative mate exists, the game continues",
            position: "1b2k^3/8/8/8/8/8/8/3rK^2B / W/w",
            half_move: mv("e1", "d1"),
            expect_legal: true,
            expect_status: Some("ongoing"),
        },
        LegalityCase {
            id: "legality.deadposition-chess-two-knights-live",
            note: "chess: King + two Knights versus King is NOT dead (the criterion is possibility, not forceability — a cooperative mate exists), the game continues",
            position: "4k^3/8/8/8/8/8/8/3rK^1NN / W/w",
            half_move: mv("e1", "d1"),
            expect_legal: true,
            expect_status: Some("ongoing"),
        },
        LegalityCase {
            id: "legality.nomove-jammed-side-without-check",
            note: "second is fully jammed (pawns on/against their last rank, king walled in by them): an empty PSEUDO-legal set — the nomove draw, not a stalemate",
            position: "7K^/8/8/8/8/8/pp6/k^p6 / W/w",
            half_move: mv("h8", "g8"),
            expect_legal: true,
            expect_status: Some("nomove"),
        },
        LegalityCase {
            id: "legality.nomove-jammed-side-in-check",
            note: "the knight checks the jammed king: still the nomove draw — an empty pseudo-legal set outranks checkmate (terminal::classify)",
            position: "7K^/8/8/8/8/8/pp6/k^p2N3 / W/w",
            half_move: mv("e1", "c2"),
            expect_legal: true,
            expect_status: Some("nomove"),
        },
        LegalityCase {
            id: "legality.en-passant-capture-resolves-check",
            note: "the just-double-stepped -p checks the King; the en-passant capture removes the checker and is legal — pins the marker-reading escape (ADR-0010 finding 3)",
            position: "7k^/8/8/3-pP3/2K^5/8/8/8 / W/w",
            half_move: mv("e5", "d6"),
            expect_legal: true,
            expect_status: Some("ongoing"),
        },
    ]
}

struct ResolvedLegality {
    case: LegalityCase,
    evaluation: Evaluation,
}

fn resolve_legality() -> Result<Vec<ResolvedLegality>, String> {
    let mut out = Vec::new();
    for case in legality_cases() {
        let evaluation = evaluate(case.position, &case.half_move)
            .map_err(|error| format!("{}: {error}", case.id))?;
        if evaluation.legal != case.expect_legal {
            return Err(format!(
                "{}: engine says legal={} (status {}), intent was legal={}",
                case.id, evaluation.legal, evaluation.status, case.expect_legal
            ));
        }
        if let Some(expected) = case.expect_status {
            if evaluation.status != expected {
                return Err(format!(
                    "{}: engine status \"{}\", intent was \"{expected}\"",
                    case.id, evaluation.status
                ));
            }
        }
        out.push(ResolvedLegality { case, evaluation });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Category D — terminality scenarios
// ---------------------------------------------------------------------------

struct ScenarioPly {
    id: String,
    seat: &'static str,
    step: u32,
    half_move: MoveSpec,
    timed_at: i64,
}

struct ScenarioCase {
    id: &'static str,
    note: &'static str,
    position: &'static str,
    t0: i64,
    cutoff: i64,
    plies: Vec<ScenarioPly>,
    /// Number of plies expected in the canonical chain (the rest are void).
    expect_chain_len: usize,
    /// The natural termination at the chain's tip (`None` = still ongoing).
    expect_termination: Option<&'static str>,
}

/// Interleave a scenario's move list into timed plies (seat alternates from
/// `first`; step is each seat's own ordinal; timings ascend by 10).
fn plies_from_moves(moves: Vec<MoveSpec>) -> Vec<ScenarioPly> {
    moves
        .into_iter()
        .enumerate()
        .map(|(index, half_move)| {
            let ordinal = index + 1;
            ScenarioPly {
                id: format!("m{ordinal}"),
                seat: if ordinal % 2 == 1 { "first" } else { "second" },
                step: u32::try_from(ordinal.div_ceil(2)).expect("small ordinal"),
                half_move,
                timed_at: i64::try_from(ordinal).expect("small ordinal") * 10,
            }
        })
        .collect()
}

/// The rook/king shuttle over the ROOK_KING position: every fourth half-move
/// returns to the initial position; the 8th reaches its third occurrence.
fn shuttle_moves() -> Vec<MoveSpec> {
    vec![
        mv("a1", "a2"),
        mv("e8", "d8"),
        mv("a2", "a1"),
        mv("d8", "e8"),
        mv("a1", "a2"),
        mv("e8", "d8"),
        mv("a2", "a1"),
        mv("d8", "e8"),
        mv("a1", "a2"), // void — the game ended on the 8th half-move
    ]
}

/// A boustrophedon rook walk over files a–f, ranks 3–6 (24 squares), starting
/// at `start` and cycling back to it: one square per move, paths always clear.
fn rook_cycle(start: &str) -> Vec<(String, String)> {
    let files = ["a", "b", "c", "d", "e", "f"];
    let mut squares: Vec<String> = Vec::new();
    for (row, rank) in ["3", "4", "5", "6"].iter().enumerate() {
        let ltr: Vec<&&str> = files.iter().collect();
        let ordered: Vec<&&str> = if row % 2 == 0 {
            ltr
        } else {
            files.iter().rev().collect()
        };
        for file in ordered {
            squares.push(format!("{file}{rank}"));
        }
    }
    let offset = squares
        .iter()
        .position(|square| square == start)
        .expect("start on the tour");
    squares.rotate_left(offset);
    let mut moves = Vec::new();
    for pair in squares.windows(2) {
        moves.push((pair[0].clone(), pair[1].clone()));
    }
    moves.push((squares[squares.len() - 1].clone(), squares[0].clone()));
    moves
}

/// 100 quiet half-moves reaching the move limit: `first` alternates a rook-tour
/// step (odd own-moves) with a king toggle (even own-moves), `second` shuttles
/// its king. `first_prefix` optionally replaces first's opening move (a drop, a
/// Tokin step, …) — the point of the ōgi variants: it must NOT reset the clock,
/// or the 100th half-move stops short of the limit and the vector fails.
fn movelimit_moves(first_prefix: Option<MoveSpec>) -> Vec<MoveSpec> {
    let mut tour = rook_cycle("a3").into_iter().cycle();
    let mut first_own = 0_u32;
    let mut king_at_h1 = true;
    let mut black_at_h8 = true;
    let mut moves = Vec::new();
    for half_move in 1..=100_u32 {
        if half_move % 2 == 1 {
            first_own += 1;
            if first_own == 1 {
                if let Some(prefix) = first_prefix.clone() {
                    moves.push(prefix);
                    continue;
                }
            }
            // With a prefix, the rook starts on first's SECOND own move; the
            // odd/even alternation below stays keyed to the rook's own count.
            let rook_ordinal = if first_prefix.is_some() {
                first_own - 1
            } else {
                first_own
            };
            if rook_ordinal >= 1 && rook_ordinal % 2 == 1 {
                let (from, to) = tour.next().expect("tour long enough");
                moves.push(mv(&from, &to));
            } else {
                let (from, to) = if king_at_h1 {
                    ("h1", "h2")
                } else {
                    ("h2", "h1")
                };
                king_at_h1 = !king_at_h1;
                moves.push(mv(from, to));
            }
        } else {
            let (from, to) = if black_at_h8 {
                ("h8", "g8")
            } else {
                ("g8", "h8")
            };
            black_at_h8 = !black_at_h8;
            moves.push(mv(from, to));
        }
    }
    // The void 101st half-move belongs to `first`: a legal-looking quiet king
    // move the harnesses never apply — that exclusion is the assertion.
    moves.push(mv("h8", "g8")); // placeholder shape; replaced just below
    let void_index = moves.len() - 1;
    moves[void_index] = if king_at_h1 {
        mv("h1", "h2")
    } else {
        mv("h2", "h1")
    };
    moves
}

/// 100 half-moves whose LAST is a mating rook lift — the checkmate must outrank
/// the simultaneously reached move limit. `second` walks its rook down a 23-step
/// snake (f6 → … → f3) on its even own-moves 1–23, toggles its king otherwise,
/// and mates with Rf3–f1 on its 50th; `first` only toggles its walled king.
fn mate_on_hundredth_moves() -> Vec<MoveSpec> {
    let files = ["f", "e", "d", "c", "b", "a"];
    let mut snake: Vec<String> = Vec::new();
    for (row, rank) in ["6", "5", "4", "3"].iter().enumerate() {
        let ordered: Vec<&&str> = if row % 2 == 0 {
            files.iter().collect()
        } else {
            files.iter().rev().collect()
        };
        for file in ordered {
            snake.push(format!("{file}{rank}"));
        }
    }
    // snake = f6,e6,…,a6,a5,…,f5,f4,…,a4,a3,…,f3 — 24 squares, 23 steps.
    let mut steps = Vec::new();
    for pair in snake.windows(2) {
        steps.push((pair[0].clone(), pair[1].clone()));
    }
    let mut tour = steps.into_iter();
    let mut white_at_h1 = true;
    let mut black_at_h8 = true;
    let mut second_own = 0_u32;
    let mut moves = Vec::new();
    for half_move in 1..=100_u32 {
        if half_move % 2 == 1 {
            let (from, to) = if white_at_h1 {
                ("h1", "g1")
            } else {
                ("g1", "h1")
            };
            white_at_h1 = !white_at_h1;
            moves.push(mv(from, to));
        } else {
            second_own += 1;
            if second_own == 50 {
                moves.push(mv("f3", "f1")); // the mating lift, on the 100th half-move
            } else if second_own % 2 == 0 && second_own <= 46 {
                let (from, to) = tour.next().expect("snake long enough");
                moves.push(mv(&from, &to));
            } else {
                let (from, to) = if black_at_h8 {
                    ("h8", "g8")
                } else {
                    ("g8", "h8")
                };
                black_at_h8 = !black_at_h8;
                moves.push(mv(from, to));
            }
        }
    }
    moves.push(mv("h1", "g1")); // void — the game ended on the mating 100th
    moves
}

fn scenario_cases() -> Vec<ScenarioCase> {
    // ROOK_KING: white Rook a1, white King e1, black King e8; white to move.
    const ROOK_KING: &str = "4k^3/8/8/8/8/8/8/R3K^3 / W/w";
    let mut cases = Vec::new();

    // 1 — repetition truncates the chain; the initial position counts as its
    //     first occurrence (two returns suffice).
    cases.push(ScenarioCase {
        id: "scenario.repetition-truncates-the-chain",
        note: "rook/king shuttle: every fourth half-move returns to the initial position (which counts as occurrence one), so the 8th reaches the third occurrence and the game ends there; the 9th ply is legal and canonically timed — and void",
        position: ROOK_KING,
        t0: 0,
        cutoff: 1_000,
        plies: plies_from_moves(shuttle_moves()),
        expect_chain_len: 8,
        expect_termination: Some("repetition"),
    });

    // 2 — a NON-initial position repeated three times.
    let mut non_initial = vec![mv("a1", "a2")]; // reach Q once…
    for _ in 0..2 {
        // …then two four-cycles around Q.
        non_initial.push(mv("e8", "d8"));
        non_initial.push(mv("a2", "a3"));
        non_initial.push(mv("d8", "e8"));
        non_initial.push(mv("a3", "a2"));
    }
    non_initial.push(mv("e8", "d8")); // void
    cases.push(ScenarioCase {
        id: "scenario.repetition-of-a-non-initial-position",
        note: "the repeated position is created on the 1st half-move and revisited by two four-cycles: third occurrence on the 9th half-move; the 10th ply is void",
        position: ROOK_KING,
        t0: 0,
        cutoff: 1_000,
        plies: plies_from_moves(non_initial),
        expect_chain_len: 9,
        expect_termination: Some("repetition"),
    });

    // 3 — the terminating ply is a PREMOVE (anterior to its slot's boundary).
    let mut premove = plies_from_moves(shuttle_moves());
    premove[7].timed_at = 65; // m7 @70 is the boundary; m8 @65 is anterior
    cases.push(ScenarioCase {
        id: "scenario.repetition-lands-on-a-premove",
        note: "same shuttle, but the terminating 8th half-move is timed BEFORE its slot's boundary (the 7th's timing): selected from the anterior window as a premove, it still closes the chain on the third occurrence",
        position: ROOK_KING,
        t0: 0,
        cutoff: 1_000,
        plies: premove,
        expect_chain_len: 8,
        expect_termination: Some("repetition"),
    });

    // 4 — the capture that leaves the two lone royals closes the chain.
    cases.push(ScenarioCase {
        id: "scenario.insufficiency-closes-the-chain",
        note: "the checking rook is captured by the king, leaving the two lone royals: an immediate insufficiency draw ON that ply — the second player's legal reply is void",
        position: "4k^3/8/8/8/8/8/8/3rK^3 / W/w",
        t0: 0,
        cutoff: 1_000,
        plies: plies_from_moves(vec![mv("e1", "d1"), mv("e8", "d8")]),
        expect_chain_len: 1,
        expect_termination: Some("insufficient"),
    });

    // 4b — the chess-only material set: the capture that leaves K+B vs K closes
    //      the chain just like the lone-royals case.
    cases.push(ScenarioCase {
        id: "scenario.deadposition-chess-kb-closes-the-chain",
        note: "the checking rook is captured by the king, leaving King + Bishop versus King: an immediate dead-position draw ON that ply (rules-of-chess §Dead-Position Detection) — the second player's legal reply is void",
        position: "4k^3/8/8/8/8/8/8/3rK^2B / W/w",
        t0: 0,
        cutoff: 1_000,
        plies: plies_from_moves(vec![mv("e1", "d1"), mv("e8", "d8")]),
        expect_chain_len: 1,
        expect_termination: Some("insufficient"),
    });

    // 5 — the 100-half-move limit, reached by a capture-free, foot-soldier-free
    //     rook tour that never repeats a position three times.
    cases.push(ScenarioCase {
        id: "scenario.movelimit-on-the-hundredth-half-move",
        note: "100 quiet half-moves (rook tour + king shuttles, no capture, no foot-soldier move, no third occurrence): the move-limit draw lands exactly on the 100th; the 101st ply is void",
        position: "7k^/8/8/8/8/R7/8/7K^ / W/w",
        t0: 0,
        cutoff: 10_000,
        plies: plies_from_moves(movelimit_moves(None)),
        expect_chain_len: 100,
        expect_termination: Some("movelimit"),
    });

    // 6 — a mate delivered ON the 100th half-move outranks the move limit.
    cases.push(ScenarioCase {
        id: "scenario.mate-on-the-hundredth-outranks-movelimit",
        note: "the 100th half-move both reaches the move-limit counter and mates the walled king: checkmate is decisive and outranks the background draw (terminal::classify)",
        position: "7k^/8/5r2/8/8/8/6+P+P/7K^ / W/w",
        t0: 0,
        cutoff: 10_000,
        plies: plies_from_moves(mate_on_hundredth_moves()),
        expect_chain_len: 100,
        expect_termination: Some("checkmate"),
    });

    // 7 — an ōgi DROP does not reset the 100-half-move counter.
    cases.push(ScenarioCase {
        id: "scenario.ogi-drop-does-not-reset-the-counter",
        note: "first opens with a Fu drop (which increments, never resets, the counter) and the limit still lands exactly on the 100th half-move — a reset-on-drop bug would leave the counter at 99 and this vector unterminated",
        position: "7k^/8/8/8/8/R7/8/7K^ F/ J/j",
        t0: 0,
        cutoff: 10_000,
        plies: plies_from_moves(movelimit_moves(Some(drop_mv("b1", "fu")))),
        expect_chain_len: 100,
        expect_termination: Some("movelimit"),
    });

    // 8 — a Tokin move does not reset the counter (it is no foot soldier).
    cases.push(ScenarioCase {
        id: "scenario.ogi-tokin-move-does-not-reset-the-counter",
        note: "first opens by stepping its Tokin (the promoted Fu — NOT a foot soldier, so no reset) and the limit still lands exactly on the 100th half-move",
        position: "7k^/8/8/8/8/R7/8/3T3K^ / J/j",
        t0: 0,
        cutoff: 10_000,
        plies: plies_from_moves(movelimit_moves(Some(mv("d1", "c1")))),
        expect_chain_len: 100,
        expect_termination: Some("movelimit"),
    });

    cases
}

/// Replays a scenario sequentially through `kernel::step` (one candidate per
/// slot, so the canonical chain is the sequence up to the first terminal) and
/// checks the designer's intent: ongoing before the terminal ply, the expected
/// terminal status ON it, and at least one void ply after it.
fn verify_scenario(case: &ScenarioCase) -> Result<(), String> {
    let position = Position::parse(case.position)
        .map_err(|error| format!("{}: invalid FEEN: {error:?}", case.id))?;
    let mut state = Some(SessionState::start(
        position,
        neutral_time_control(),
        Timestamp::from_unix(0),
    ));
    for (index, ply) in case.plies.iter().enumerate() {
        let Some(current) = state.take() else {
            return Err(format!(
                "{}: ply {} lies beyond the terminal ply — only the first ply after the end may be emitted as void",
                case.id,
                index + 1
            ));
        };
        let content = content_of(&ply.half_move);
        let parsed = Move::parse(&content)
            .map_err(|error| format!("{}: ply {}: unparseable ({error:?})", case.id, index + 1))?;
        let result = step(current, &parsed, Timestamp::from_unix(0));
        let status = verdict_status(&result.outcome.verdict);
        let ordinal = index + 1;
        if ordinal < case.expect_chain_len {
            if status != "ongoing" {
                return Err(format!(
                    "{}: ply {ordinal} unexpectedly terminal ({status}) before the intended end at {}",
                    case.id, case.expect_chain_len
                ));
            }
            state = result.next;
        } else if ordinal == case.expect_chain_len {
            match case.expect_termination {
                Some(expected) => {
                    if status != expected {
                        return Err(format!(
                            "{}: terminal ply {ordinal} has status \"{status}\", intent was \"{expected}\"",
                            case.id
                        ));
                    }
                    if result.next.is_some() {
                        return Err(format!(
                            "{}: ply {ordinal} was meant to terminate but the game continues",
                            case.id
                        ));
                    }
                    state = None;
                }
                None => {
                    state = result.next;
                }
            }
        }
        if ordinal == case.expect_chain_len && case.expect_termination.is_some() {
            // Everything after the terminal ply is void; it is never applied, but
            // the downstream harnesses still parse it — check it is well-formed.
            if case.plies.len() == case.expect_chain_len {
                return Err(format!(
                    "{}: add a void ply after the terminal one — its exclusion IS the truncation assertion",
                    case.id
                ));
            }
            for void in &case.plies[case.expect_chain_len..] {
                let void_content = content_of(&void.half_move);
                Move::parse(&void_content).map_err(|error| {
                    format!("{}: void ply {}: unparseable ({error:?})", case.id, void.id)
                })?;
            }
            return Ok(());
        }
    }
    if case.expect_termination.is_some() {
        return Err(format!(
            "{}: never reached the intended terminal ply",
            case.id
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Emission (styles mirror the existing corpus files)
// ---------------------------------------------------------------------------

fn json_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Category A additions — 2-space indentation, multi-line move arrays, key
/// order id/note/position/move/legal/result/status (as `legality.json`).
fn emit_legality(resolved: &[ResolvedLegality]) -> String {
    let mut out = String::new();
    out.push_str("{\n  \"category\": \"legality\",\n  \"note\": \"ADR-0010 additions — merge into legality.json (engine-generated by examples/gen_vectors.rs)\",\n  \"vectors\": [\n");
    for (index, entry) in resolved.iter().enumerate() {
        let case = &entry.case;
        let src = match &case.half_move.src {
            Some(s) => format!("\"{s}\""),
            None => "null".to_owned(),
        };
        let actor = match &case.half_move.actor {
            Some(a) => format!("\"{a}\""),
            None => "null".to_owned(),
        };
        let _ = write!(
            out,
            "    {{\n      \"id\": \"{}\",\n      \"note\": \"{}\",\n      \"position\": \"{}\",\n      \"move\": [\n        {src},\n        \"{}\",\n        {actor}\n      ],\n      \"legal\": {},\n      \"result\": \"{}\",\n      \"status\": \"{}\"\n    }}",
            json_escape(case.id),
            json_escape(case.note),
            json_escape(case.position),
            json_escape(&case.half_move.dst),
            entry.evaluation.legal,
            json_escape(&entry.evaluation.result),
            json_escape(&entry.evaluation.status),
        );
        out.push_str(if index + 1 < resolved.len() {
            ",\n"
        } else {
            "\n"
        });
    }
    out.push_str("  ]\n}\n");
    out
}

/// Category D additions — tab indentation, single-line ply objects (as
/// `scenarios.json`), plus the ADR-0010 `expectedTermination` field.
fn emit_scenarios(cases: &[ScenarioCase]) -> String {
    let mut out = String::new();
    out.push_str("{\n\t\"category\": \"scenario\",\n\t\"note\": \"ADR-0010 additions — merge into scenarios.json and bump its version (engine-generated by examples/gen_vectors.rs)\",\n\t\"vectors\": [\n");
    for (index, case) in cases.iter().enumerate() {
        let _ = write!(
            out,
            "\t\t{{\n\t\t\t\"id\": \"{}\",\n\t\t\t\"note\": \"{}\",\n\t\t\t\"position\": \"{}\",\n\t\t\t\"t0\": {},\n\t\t\t\"cutoff\": {},\n\t\t\t\"plies\": [\n",
            json_escape(case.id),
            json_escape(case.note),
            json_escape(case.position),
            case.t0,
            case.cutoff,
        );
        for (ply_index, ply) in case.plies.iter().enumerate() {
            let src = match &ply.half_move.src {
                Some(s) => format!("\"{s}\""),
                None => "null".to_owned(),
            };
            let actor = match &ply.half_move.actor {
                Some(a) => format!("\"{a}\""),
                None => "null".to_owned(),
            };
            let _ = write!(
                out,
                "\t\t\t\t{{ \"id\": \"{}\", \"seat\": \"{}\", \"step\": {}, \"move\": [{src}, \"{}\", {actor}], \"timedAt\": {} }}",
                json_escape(&ply.id),
                ply.seat,
                ply.step,
                json_escape(&ply.half_move.dst),
                ply.timed_at,
            );
            out.push_str(if ply_index + 1 < case.plies.len() {
                ",\n"
            } else {
                "\n"
            });
        }
        let chain: Vec<String> = case.plies[..case.expect_chain_len]
            .iter()
            .map(|ply| format!("\"{}\"", ply.id))
            .collect();
        let termination = match case.expect_termination {
            Some(status) => format!("{{ \"status\": \"{status}\" }}"),
            None => "null".to_owned(),
        };
        let _ = write!(
            out,
            "\t\t\t],\n\t\t\t\"expectedChain\": [{}],\n\t\t\t\"expectedTermination\": {termination}\n\t\t}}",
            chain.join(", ")
        );
        out.push_str(if index + 1 < cases.len() { ",\n" } else { "\n" });
    }
    out.push_str("\t]\n}\n");
    out
}

// ---------------------------------------------------------------------------

fn run() -> Result<(), String> {
    let out_dir = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("corpus-additions"), PathBuf::from);

    let legality = resolve_legality()?;
    let scenarios = scenario_cases();
    for case in &scenarios {
        verify_scenario(case)?;
    }

    std::fs::create_dir_all(&out_dir)
        .map_err(|error| format!("creating {}: {error}", out_dir.display()))?;
    let legality_path = out_dir.join("legality-additions.json");
    let scenarios_path = out_dir.join("scenarios-additions.json");
    std::fs::write(&legality_path, emit_legality(&legality))
        .map_err(|error| format!("writing {}: {error}", legality_path.display()))?;
    std::fs::write(&scenarios_path, emit_scenarios(&scenarios))
        .map_err(|error| format!("writing {}: {error}", scenarios_path.display()))?;

    println!(
        "verified and wrote {} legality vector(s) and {} scenario(s) to {}",
        legality.len(),
        scenarios.len(),
        out_dir.display()
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("gen_vectors: {message}");
            ExitCode::FAILURE
        }
    }
}
