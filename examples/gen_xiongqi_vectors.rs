//! Xiongqi legality-coverage generator — the gap named in the conformance
//! README's §Remaining gaps: `xiongqi` legality coverage existed (8 vectors)
//! but was concentrated on en passant and castling, so **nothing exercised
//! Bear or Empress movement, the Soldier's river crossing, or promotion by
//! choice**. These sixteen cases close exactly that.
//!
//! Same provenance rule as `gen_vectors.rs`: inputs (positions, moves) are
//! curated here and carry the designer's *intent*; expected values are
//! computed by `kernel::step`, and generation FAILS if the engine disagrees.
//!
//! Usage: `cargo run --release --example gen_xiongqi_vectors [-- <out-dir>]`
//! (default `corpus-additions/`). Emits `legality-xiongqi-additions.json` in
//! the 2-space style of `legality.json`.

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
use sashite_sanki_engine::kernel::step::{step, StepResult};
use sashite_sanki_engine::position::Position;

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

fn mv_actor(src: &str, dst: &str, actor: &str) -> MoveSpec {
    MoveSpec {
        src: Some(src.to_owned()),
        dst: dst.to_owned(),
        actor: Some(actor.to_owned()),
    }
}

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

fn neutral_time_control() -> TimeControl {
    let period = Period::new(Duration::from_secs(3_600), None, None).expect("valid period");
    TimeControl::new(period, Vec::new())
}

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

fn evaluate(feen: &str, half_move: &MoveSpec) -> Result<Evaluation, String> {
    let position =
        Position::parse(feen).map_err(|error| format!("invalid FEEN {feen}: {error:?}"))?;
    let content = content_of(half_move);
    let parsed = Move::parse(&content)
        .map_err(|error| format!("move did not parse {content}: {error:?}"))?;
    let state = SessionState::start(position, neutral_time_control(), Timestamp::from_unix(0));
    match step(state, &parsed, Timestamp::from_unix(0)) {
        StepResult::Illegal { state, .. } => Ok(Evaluation {
            legal: false,
            status: "rejected".to_owned(),
            result: state.position().to_feen(),
        }),
        StepResult::Advanced { outcome, .. } => Ok(Evaluation {
            legal: true,
            status: verdict_status(&outcome.verdict),
            result: outcome.position,
        }),
    }
}

struct LegalityCase {
    id: &'static str,
    note: &'static str,
    position: &'static str,
    half_move: MoveSpec,
    expect_legal: bool,
    expect_status: Option<&'static str>,
}

fn legality_cases() -> Vec<LegalityCase> {
    vec![
        // ---- Bear (B) — the diagonal slider that replaces the Bishop -------
        LegalityCase {
            id: "legality.xiongqi-bear-diagonal-slide",
            note: "the Bear (熊) slides any number of squares diagonally, exactly as a Western Bishop — here two squares within Red's own half",
            position: "g^7/8/8/8/8/8/8/2B1G^3 / C/c",
            half_move: mv("c1", "e3"),
            expect_legal: true,
            expect_status: Some("ongoing"),
        },
        LegalityCase {
            id: "legality.xiongqi-bear-crosses-the-river",
            note: "the river restricts ONLY Soldiers: the Bear slides straight across it, unlike the xiàngqí Elephant it replaces, which may never leave its own half",
            position: "g^7/8/8/8/8/8/8/2B1G^3 / C/c",
            half_move: mv("c1", "h6"),
            expect_legal: true,
            expect_status: Some("ongoing"),
        },
        LegalityCase {
            id: "legality.xiongqi-bear-cannot-jump",
            note: "the Bear is a slider, not a jumper: an occupied square on the diagonal blocks it",
            position: "g^7/8/8/8/8/4s3/8/2B1G^3 / C/c",
            half_move: mv("c1", "h6"),
            expect_legal: false,
            expect_status: None,
        },
        LegalityCase {
            id: "legality.xiongqi-bear-has-no-orthogonal-move",
            note: "the Bear's domain is the diagonal alone — the orthogonal slide belongs to the Chariot",
            position: "g^7/8/8/8/8/8/8/2B1G^3 / C/c",
            half_move: mv("c1", "c5"),
            expect_legal: false,
            expect_status: None,
        },
        // ---- Empress (E) — Chariot + Knight, the piece with no diagonal ----
        LegalityCase {
            id: "legality.xiongqi-empress-orthogonal-slide",
            note: "the Empress (妃/騛) combines Chariot and Knight; here the Chariot component slides the full file. The lone black Soldier on h4 is deliberate: without a second black piece, an Empress on d7 covers both a7 (rank) and b8 (knight jump) and the bare General is STALEMATED — the vector would then test the terminal classifier rather than the slide",
            position: "g^7/8/8/8/7s/8/8/3EG^3 / C/c",
            half_move: mv("d1", "d7"),
            expect_legal: true,
            expect_status: Some("ongoing"),
        },
        LegalityCase {
            id: "legality.xiongqi-empress-knight-jump-over-occupied-squares",
            note: "the Knight component jumps: the Empress reaches f2 from d1 with BOTH intervening squares occupied (its own General on e1, its own Soldier on e2)",
            position: "g^7/8/8/8/8/8/3SS3/3EG^3 / C/c",
            half_move: mv("d1", "f2"),
            expect_legal: true,
            expect_status: Some("ongoing"),
        },
        LegalityCase {
            id: "legality.xiongqi-empress-orthogonal-slide-cannot-jump",
            note: "the Chariot component is a slider: a piece on the file blocks it (only the Knight component jumps)",
            position: "g^7/8/8/8/3s4/8/8/3EG^3 / C/c",
            half_move: mv("d1", "d7"),
            expect_legal: false,
            expect_status: None,
        },
        LegalityCase {
            id: "legality.xiongqi-empress-has-no-diagonal-slide",
            note: "the Empress is Chariot+Knight, NOT Chariot+Bear: it has no diagonal slide at all — the single sharpest difference from the Western Queen it replaces",
            position: "g^7/8/8/8/8/8/8/3EG^3 / C/c",
            half_move: mv("d1", "g4"),
            expect_legal: false,
            expect_status: None,
        },
        // ---- Soldier (S) — the river grants the sideways step --------------
        LegalityCase {
            id: "legality.xiongqi-soldier-sideways-before-the-river-is-illegal",
            note: "a Red Soldier on rank 3 has not crossed the river (the midline between ranks 4 and 5), so it has no sideways step yet",
            position: "g^7/8/8/8/8/3S4/8/4G^3 / C/c",
            half_move: mv("d3", "e3"),
            expect_legal: false,
            expect_status: None,
        },
        LegalityCase {
            id: "legality.xiongqi-soldier-sideways-after-crossing-is-legal",
            note: "past the river (Red on ranks 5-8) the Soldier gains the one-square sideways step onto an empty square — the river's only effect in the whole game",
            position: "g^7/8/3S4/8/8/8/8/4G^3 / C/c",
            half_move: mv("d6", "e6"),
            expect_legal: true,
            expect_status: Some("ongoing"),
        },
        LegalityCase {
            id: "legality.xiongqi-soldier-never-captures-diagonally",
            note: "the Soldier captures straight forward only — never diagonally, at any point of the board, crossed river or not (unlike the chess Pawn)",
            position: "g^7/4s3/3S4/8/8/8/8/4G^3 / C/c",
            half_move: mv("d6", "e7"),
            expect_legal: false,
            expect_status: None,
        },
        LegalityCase {
            id: "legality.xiongqi-soldier-double-step",
            note: "a Soldier still carrying the `+` prefix may advance two squares straight forward when both squares are empty",
            position: "g^7/8/8/8/8/8/3+S4/4G^3 / C/c",
            half_move: mv("d2", "d4"),
            expect_legal: true,
            expect_status: Some("ongoing"),
        },
        // ---- Promotion by choice -------------------------------------------
        LegalityCase {
            id: "legality.xiongqi-soldier-promotes-to-empress",
            note: "promotion is mandatory on the last rank and the target is the player's genuine CHOICE among four pieces; the move record must carry it as the actor",
            position: "8/3S4/8/g^7/8/8/8/4G^3 / C/c",
            half_move: mv_actor("d7", "d8", "empress"),
            expect_legal: true,
            expect_status: Some("ongoing"),
        },
        LegalityCase {
            id: "legality.xiongqi-soldier-promotes-to-knight",
            note: "the same Soldier on the same square promotes to a Knight instead — proving the choice is real and not a fixed transformation (contrast the ōgi Fu, whose Tokin promotion is forced and actorless)",
            position: "8/3S4/8/g^7/8/8/8/4G^3 / C/c",
            half_move: mv_actor("d7", "d8", "knight"),
            expect_legal: true,
            expect_status: Some("ongoing"),
        },
        LegalityCase {
            id: "legality.xiongqi-soldier-promotion-to-general-is-illegal",
            note: "the four targets are Chariot, Knight, Bear and Empress: a Soldier may never become the royal piece",
            position: "8/3S4/8/g^7/8/8/8/4G^3 / C/c",
            half_move: mv_actor("d7", "d8", "general"),
            expect_legal: false,
            expect_status: None,
        },
        LegalityCase {
            id: "legality.xiongqi-soldier-promotion-to-soldier-is-illegal",
            note: "promotion is mandatory: the Soldier may not decline it by naming its own kind",
            position: "8/3S4/8/g^7/8/8/8/4G^3 / C/c",
            half_move: mv_actor("d7", "d8", "soldier"),
            expect_legal: false,
            expect_status: None,
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

fn json_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out
}

fn emit_legality(resolved: &[ResolvedLegality]) -> String {
    let mut out = String::new();
    out.push_str("{\n  \"category\": \"legality\",\n  \"note\": \"xiongqi movement, river and promotion-by-choice coverage — merge into legality.json (engine-generated by examples/gen_xiongqi_vectors.rs)\",\n  \"vectors\": [\n");
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
            "    {{\n      \"id\": \"{}\",\n      \"note\": \"{}\",\n      \"position\": \"{}\",\n      \"move\": [\n        {src},\n        \"{}\",\n        {actor}\n      ],\n      \"legal\": {}",
            json_escape(case.id),
            json_escape(case.note),
            json_escape(case.position),
            json_escape(&case.half_move.dst),
            entry.evaluation.legal,
        );
        if entry.evaluation.legal {
            let _ = write!(
                out,
                ",\n      \"result\": \"{}\",\n      \"status\": \"{}\"\n    }}",
                json_escape(&entry.evaluation.result),
                json_escape(&entry.evaluation.status),
            );
        } else {
            out.push_str("\n    }");
        }
        out.push_str(if index + 1 < resolved.len() {
            ",\n"
        } else {
            "\n"
        });
    }
    out.push_str("  ]\n}\n");
    out
}

fn main() -> ExitCode {
    let out_dir = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("corpus-additions"), PathBuf::from);

    let resolved = match resolve_legality() {
        Ok(resolved) => resolved,
        Err(error) => {
            eprintln!("intent/engine disagreement — nothing emitted:\n  {error}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(error) = std::fs::create_dir_all(&out_dir) {
        eprintln!("cannot create {}: {error}", out_dir.display());
        return ExitCode::FAILURE;
    }
    let path = out_dir.join("legality-xiongqi-additions.json");
    if let Err(error) = std::fs::write(&path, emit_legality(&resolved)) {
        eprintln!("cannot write {}: {error}", path.display());
        return ExitCode::FAILURE;
    }
    println!(
        "{} vectors verified and written to {}",
        resolved.len(),
        path.display()
    );
    ExitCode::SUCCESS
}
