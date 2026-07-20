//! Shared conformance vectors — clock arithmetic (category E).
//!
//! Drives `tests/conformance/clock.json` (a vendored copy of the shared corpus at
//! `web-specs.md/nostr/conformance`) through [`sashite_sanki_engine::clock::tick`].
//! Each vector is one tick: a time control (kind-6420 period triples), the mover's
//! clock, the ply's elapsed seconds, and the expected outcome — the updated clock
//! or the flag. The TypeScript client runs the same JSON through its `clock.ts`
//! port, so the Fischer/quota/rollover arithmetic of
//! `time-accounting-sanki.md` §Worked examples cannot drift between what a client
//! displays and what the arbiter (through this crate's kernel) rules.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use sashite_sanki_engine::clock::{tick, Tick};
use sashite_sanki_engine::domain::time::Duration;
use sashite_sanki_engine::domain::time_control::{Clock, Period, TimeControl};

#[derive(serde::Deserialize)]
struct Corpus {
    vectors: Vec<ClockVector>,
}

#[derive(serde::Deserialize)]
struct ClockVector {
    id: String,
    /// Period triples `[duration, increment, plies]` (kind-6420 order).
    #[serde(rename = "timeControl")]
    time_control: Vec<(u64, Option<u64>, Option<u32>)>,
    clock: ClockState,
    elapsed: u64,
    expected: Expected,
}

#[derive(serde::Deserialize)]
struct ClockState {
    remaining: u64,
    period: usize,
    #[serde(rename = "pliesInPeriod")]
    plies_in_period: u32,
}

#[derive(serde::Deserialize)]
#[serde(tag = "kind")]
enum Expected {
    #[serde(rename = "flagged")]
    Flagged,
    #[serde(rename = "continued")]
    Continued { clock: ClockState },
}

/// The vendored clock corpus.
fn corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/conformance/clock.json")
}

fn time_control_of(vector: &ClockVector) -> TimeControl {
    let periods = vector
        .time_control
        .iter()
        .map(|&(duration, increment, plies)| {
            Period::new(
                Duration::from_secs(duration),
                increment.map(Duration::from_secs),
                plies,
            )
            .expect("vector period must be valid")
        })
        .collect();
    TimeControl::from_periods(periods).expect("vector time control must be non-empty")
}

const fn clock_of(state: &ClockState) -> Clock {
    Clock::new(
        Duration::from_secs(state.remaining),
        state.period,
        state.plies_in_period,
    )
}

#[test]
fn clock_conformance() {
    let path = corpus_path();
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let corpus: Corpus = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("cannot parse {}: {e}", path.display()));

    let mut failures = Vec::new();
    for vector in &corpus.vectors {
        let control = time_control_of(vector);
        let actual = tick(
            &control,
            clock_of(&vector.clock),
            Duration::from_secs(vector.elapsed),
        );
        match (&vector.expected, actual) {
            (Expected::Flagged, Tick::Flagged) => {}
            (Expected::Continued { clock }, Tick::Continued(got)) => {
                let want = clock_of(clock);
                if got != want {
                    failures.push(format!(
                        "{}: clock diverges\n  expected {want:?}\n  actual   {got:?}",
                        vector.id
                    ));
                }
            }
            (Expected::Flagged, Tick::Continued(got)) => {
                failures.push(format!("{}: expected a flag, got {got:?}", vector.id));
            }
            (Expected::Continued { .. }, Tick::Flagged) => {
                failures.push(format!("{}: flagged, expected a continuation", vector.id));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} clock conformance divergence(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(
        !corpus.vectors.is_empty(),
        "no clock conformance vectors found at {}",
        path.display()
    );
}
