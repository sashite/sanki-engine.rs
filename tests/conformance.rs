//! Shared conformance vectors — legality (ADR-0002 forgiving premoves).
//!
//! Drives `tests/conformance/legality.json` (a vendored copy of the shared corpus at
//! `web-specs.md/nostr/conformance`) through the **full rule system** — the same
//! `kernel::step` path the arbiter uses, so "legal" here means *legal per the rules*
//! (including ōgi uchifuzume), not merely mechanically resolvable by `engine::validate`.
//! For each vector the verdict's `illegalmove` must match `legal == false`; every legal
//! vector's `result` (canonical FEEN) and `status` must match the step outcome. The
//! TypeScript client runs the same JSON, so the two implementations cannot drift.
//!
//! The `selection.json` vectors need the arbiter's selection API and are exercised in
//! the `sashite-sanki-arbiter` crate, not here. Mirrors the `step` replay of
//! `differential.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use sashite_sanki_engine::domain::half_move::Move;
use sashite_sanki_engine::domain::outcome::Verdict;
use sashite_sanki_engine::domain::time::{Duration, Timestamp};
use sashite_sanki_engine::domain::time_control::{Period, TimeControl};
use sashite_sanki_engine::kernel::state::SessionState;
use sashite_sanki_engine::kernel::step::step;
use sashite_sanki_engine::position::Position;

#[derive(serde::Deserialize)]
struct Corpus {
    vectors: Vec<LegalityVector>,
}

#[derive(serde::Deserialize)]
struct LegalityVector {
    id: String,
    position: String,
    #[serde(rename = "move")]
    half_move: serde_json::Value,
    legal: bool,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

/// The vendored legality corpus.
fn corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/conformance/legality.json")
}

/// A time control generous enough never to flag, so the verdict reflects only the
/// rule system (each case is replayed with zero elapsed time).
fn neutral_time_control() -> TimeControl {
    let period = Period::new(Duration::from_secs(3_600), None, None).expect("valid period");
    TimeControl::new(period, Vec::new())
}

/// The corpus status string for a verdict: `ongoing`, or the terminal status.
fn verdict_status(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Ongoing => "ongoing".to_owned(),
        Verdict::Terminated { status, .. } => status.to_string(),
    }
}

#[test]
fn legality_conformance() {
    let path = corpus_path();
    let Ok(contents) = std::fs::read_to_string(&path) else {
        eprintln!(
            "conformance corpus absent ({}) — test skipped.",
            path.display()
        );
        return;
    };
    let corpus: Corpus =
        serde_json::from_str(&contents).expect("conformance/legality.json: invalid JSON");

    let mut failures: Vec<String> = Vec::new();

    for vector in &corpus.vectors {
        let position = match Position::parse(&vector.position) {
            Ok(position) => position,
            Err(error) => {
                failures.push(format!("{}: invalid FEEN ({error:?})", vector.id));
                continue;
            }
        };

        // The native JSON move is re-serialized to the `[src, dst, actor]` content.
        let content = serde_json::to_string(&vector.half_move).expect("serializing the move");
        let half_move = match Move::parse(&content) {
            Ok(half_move) => half_move,
            Err(error) => {
                // A non-parsing move is not legal; only an illegal vector may carry one.
                if vector.legal {
                    failures.push(format!(
                        "{}: move did not parse ({error:?}): {content}",
                        vector.id
                    ));
                }
                continue;
            }
        };

        // Full-rule replay via the kernel — illegality is encoded in the verdict.
        let state = SessionState::start(position, neutral_time_control(), Timestamp::from_unix(0));
        let outcome = step(state, &half_move, Timestamp::from_unix(0)).outcome;
        let actual_status = verdict_status(&outcome.verdict);
        let is_legal = actual_status != "illegalmove";

        if is_legal != vector.legal {
            failures.push(format!(
                "{}: rule-system legality = {is_legal} (status \"{actual_status}\"), expected legal = {}",
                vector.id, vector.legal
            ));
            continue;
        }
        if !vector.legal {
            continue;
        }

        // A legal move's canonical resulting FEEN and status must match the vector.
        if let Some(expected) = &vector.result {
            if outcome.position != *expected {
                failures.push(format!(
                    "{}: result FEEN diverges\n  expected {expected}\n  actual   {}",
                    vector.id, outcome.position
                ));
            }
        }
        if let Some(expected) = &vector.status {
            if &actual_status != expected {
                failures.push(format!(
                    "{}: expected status \"{expected}\", got \"{actual_status}\"",
                    vector.id
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} conformance divergence(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(
        !corpus.vectors.is_empty(),
        "no conformance vectors found at {}",
        path.display()
    );
}
