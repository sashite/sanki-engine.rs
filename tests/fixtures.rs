//! Golden transition fixtures, data-driven (JSON).
//!
//! This file's single responsibility: for each `{ feen, ply, expected }` case in a
//! `tests/fixtures/*.json` file, check that applying the `ply` to the source FEEN
//! through the public façade [`engine::apply`](sashite_sanki_engine::engine::apply) produces
//! exactly the expected destination FEEN. Unlike the inline vectors of
//! `golden_feen`, the cases live in **external** files: coverage grows by dropping
//! in a `.json`, without touching the Rust.
//!
//! # Format
//!
//! Each file is a JSON array of objects:
//!
//! ```json
//! {
//!   "name": "human-readable label",
//!   "feen": "<source FEEN>",
//!   "ply":  ["a1", "a4", null],
//!   "expected": "<canonical destination FEEN>",
//!   "status": "ongoing"
//! }
//! ```
//!
//! - `ply` is the move in SANKI format `[source, destination, actor]` (a native
//!   JSON array: `source` is `null` for a drop, `actor` names the promotion target
//!   or the dropped piece);
//! - `expected` is the canonical FEEN after the move;
//! - `status` (optional) is the **intrinsic** status of the resulting position
//!   (`ongoing`, `checkmate`, `stalemate`, `nomove`, `insufficient`).
//!
//! Every `.json` in the directory is loaded and run. If the directory is absent,
//! the test skips cleanly.

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
use sashite_sanki_engine::engine::{apply, status};
use sashite_sanki_engine::position::Position;

#[derive(serde::Deserialize)]
struct Case {
    name: String,
    feen: String,
    ply: serde_json::Value,
    expected: String,
    #[serde(default)]
    status: Option<String>,
}

/// The directory holding the fixture files.
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// The intrinsic status string of a verdict: `ongoing`, or the terminal status.
fn verdict_status(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Ongoing => "ongoing".to_owned(),
        Verdict::Terminated { status, .. } => status.to_string(),
    }
}

#[test]
fn transition_fixtures() {
    let dir = fixtures_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!(
            "fixtures directory absent ({}) — test skipped.",
            dir.display()
        );
        return;
    };

    // `.json` files, in a deterministic order.
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();

    let mut failures: Vec<String> = Vec::new();
    let mut count = 0_usize;

    for file in &files {
        let label = file.file_name().map_or_else(
            || file.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let contents = std::fs::read_to_string(file).expect("reading the fixtures file");
        let cases: Vec<Case> = serde_json::from_str(&contents)
            .unwrap_or_else(|error| panic!("{label}: invalid JSON: {error}"));

        for case in cases {
            count += 1;
            let where_ = format!("{label} / \"{}\"", case.name);

            let position = match Position::parse(&case.feen) {
                Ok(position) => position,
                Err(error) => {
                    failures.push(format!("{where_}: invalid source FEEN ({error:?})"));
                    continue;
                }
            };

            // The native JSON ply is re-serialized to the `[src, dst, actor]` content.
            let content = serde_json::to_string(&case.ply).expect("serializing the ply");
            let half_move = match Move::parse(&content) {
                Ok(half_move) => half_move,
                Err(error) => {
                    failures.push(format!("{where_}: invalid ply ({error:?}): {content}"));
                    continue;
                }
            };

            let next = match apply(&position, &half_move) {
                Ok(next) => next,
                Err(reason) => {
                    failures.push(format!("{where_}: move rejected ({reason:?})"));
                    continue;
                }
            };

            let actual = next.to_feen();
            if actual != case.expected {
                failures.push(format!(
                    "{where_}: destination FEEN diverges\n  expected  {}\n  actual    {actual}",
                    case.expected
                ));
                continue;
            }

            if let Some(expected_status) = &case.status {
                let actual_status = verdict_status(&status(&next));
                if &actual_status != expected_status {
                    failures.push(format!(
                        "{where_}: expected status \"{expected_status}\", got \"{actual_status}\""
                    ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} divergence(s) over {count} cases:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(count > 0, "no fixture case found in {}", dir.display());
}
