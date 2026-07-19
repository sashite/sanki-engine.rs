//! Differential tests vs Ruby/Elixir implementations (ignored by default).
//!
//! This file's single responsibility: to confront our rules engine with an
//! **external corpus** produced by another Sashité implementation (Ruby, Elixir,
//! …). Each case is a per-ply kernel-level transition —
//! `(position, move) -> (resulting position, status)` — replayed here via [`step`]
//! and compared against the reference values.
//!
//! # Corpus format
//!
//! A **TSV** (tab-separated) file, one line per case, four fields:
//!
//! ```text
//! <feen>\t<move>\t<resulting_feen>\t<status>
//! ```
//!
//! - `feen` — the starting position (Sanki FEEN);
//! - `move` — the Ply `content` (JSON array `[source, destination, actor]`);
//! - `resulting_feen` — the canonical position after the move; for an illegal
//!   move, this is the starting position **unchanged**;
//! - `status` — `ongoing`, a terminal status (`checkmate`, `stalemate`,
//!   `nomove`, `insufficient`, …) as rendered by the status's display, or
//!   `rejected` for an illegal move (which the kernel rejects without
//!   terminating — there is no `illegalmove` status).
//!
//! Blank lines and those starting with `#` are ignored. A FEEN contains spaces but
//! no tab; the JSON `content` contains none either: the TSV is thus unambiguous and
//! needs no JSON parser.
//!
//! # Running
//!
//! The test is `#[ignore]`: it runs only on demand. The corpus path comes from the
//! `SANKI_DIFFERENTIAL_CORPUS` environment variable, or defaults to
//! `tests/corpus/differential.tsv`. If the file is absent, the test **skips**
//! cleanly (it does not fail), so as not to penalize an environment without a
//! reference corpus.
//!
//! ```sh
//! cargo test --test differential -- --ignored
//! SANKI_DIFFERENTIAL_CORPUS=/path/to/corpus.tsv \
//!     cargo test --test differential -- --ignored
//! ```
//!
//! # Note on time
//!
//! The comparison targets the **rule system**, not the clock: each case is
//! replayed with a generous budget and zero elapsed time, so the verdict never
//! reflects a timeout, only rule conditions.

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
use sashite_sanki_engine::kernel::step::{step, StepResult};
use sashite_sanki_engine::position::Position;

/// The corpus path: the `SANKI_DIFFERENTIAL_CORPUS` override, or the default
/// `tests/corpus/differential.tsv` under the crate root.
fn corpus_path() -> PathBuf {
    if let Ok(path) = std::env::var("SANKI_DIFFERENTIAL_CORPUS") {
        return PathBuf::from(path);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/differential.tsv")
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
#[ignore = "requires an external differential corpus (see the module header)"]
fn differential_rule_engine() {
    let path = corpus_path();
    let Ok(contents) = std::fs::read_to_string(&path) else {
        eprintln!(
            "differential corpus absent ({}) — test skipped. \
             Set SANKI_DIFFERENTIAL_CORPUS or create the file.",
            path.display()
        );
        return;
    };

    let mut failures: Vec<String> = Vec::new();
    let mut count = 0_usize;

    for (index, raw) in contents.lines().enumerate() {
        let lineno = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let fields: Vec<&str> = line.split('\t').collect();
        assert!(
            fields.len() == 4,
            "line {lineno}: 4 fields expected (feen, move, resulting_feen, status), \
             {} found",
            fields.len()
        );
        let feen = fields[0];
        let move_src = fields[1];
        let expected_feen = fields[2];
        let expected_status = fields[3];
        count += 1;

        let position = match Position::parse(feen) {
            Ok(position) => position,
            Err(error) => {
                failures.push(format!("line {lineno}: invalid FEEN ({error:?}): {feen}"));
                continue;
            }
        };
        let half_move = match Move::parse(move_src) {
            Ok(half_move) => half_move,
            Err(error) => {
                failures.push(format!(
                    "line {lineno}: invalid move ({error:?}): {move_src}"
                ));
                continue;
            }
        };

        let state = SessionState::start(position, neutral_time_control(), Timestamp::from_unix(0));
        let (actual_status, actual_feen) = match step(state, &half_move, Timestamp::from_unix(0)) {
            // An illegal move is a rejection: the position is unchanged.
            StepResult::Illegal { state, .. } => {
                ("rejected".to_owned(), state.position().to_feen())
            }
            StepResult::Advanced { outcome, .. } => {
                (verdict_status(&outcome.verdict), outcome.position)
            }
        };
        if actual_status != expected_status {
            failures.push(format!(
                "line {lineno}: expected status \"{expected_status}\", got \"{actual_status}\"\n  \
                 feen={feen}\n  move={move_src}"
            ));
            continue;
        }

        if actual_feen != expected_feen {
            failures.push(format!(
                "line {lineno}: resulting FEEN diverges\n  \
                 expected  {expected_feen}\n  actual    {}\n  (feen={feen}, move={move_src})",
                actual_feen
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} divergence(s) over {count} cases:\n{}",
        failures.len(),
        failures.join("\n")
    );
    eprintln!("differential: {count} cases, no divergence.");
}
