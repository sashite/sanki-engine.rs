//! Golden vectors: FEEN canonicalization and initial positions.
//!
//! This file's single responsibility: to freeze, as known vectors, two properties
//! of the FEEN handling exposed by the crate:
//!
//! 1. **Initial positions** — the canonical starting positions of the three
//!    variants (and of a multi-variant game) are stable under `parse -> to_feen`;
//! 2. **Marker canonicalization** — after a move, the kernel recomposes the
//!    transient markers (check `-`, double-step `+`, en passant `-`, castling
//!    rights `+R`/`-R`/`R`) to produce an expected canonical FEEN.
//!
//! Added to this: marker preservation through the serialization round-trip, and
//! the rejection of syntactically invalid FEENs. This file is **self-contained**:
//! it depends on no corpus and no external reference implementation.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use sashite_sanki_engine::domain::half_move::Move;
use sashite_sanki_engine::domain::time::{Duration, Timestamp};
use sashite_sanki_engine::domain::time_control::{Period, TimeControl};
use sashite_sanki_engine::kernel::state::SessionState;
use sashite_sanki_engine::kernel::step::{step, StepResult};
use sashite_sanki_engine::position::Position;

/// Chess: royal Kings `k^`/`K^`, Rooks `-r`/`-R` (castling right retained but
/// blocked at the start by the intervening pieces), Pawns `+` eligible for the
/// double step. First to move.
const CHESS_START: &str = "-rnbqk^bn-r/+p+p+p+p+p+p+p+p/8/8/8/8/+P+P+P+P+P+P+P+P/-RNBQK^BN-R / W/w";

/// Ōgi (8×8 shōgi): Princess `i`/`I`, royal King, Fu `+f`/`+F` for the double
/// step; no castling (Rooks with no prefix). Empty hands (Sashité convention).
const OGI_START: &str = "rnbik^bnr/+f+f+f+f+f+f+f+f/8/8/8/8/+F+F+F+F+F+F+F+F/RNBIK^BNR / J/j";

/// Xiongqi (8×8 xiangqi): royal General `g^`/`G^`, Empress `e`/`E`, Bear `b`/`B`,
/// Soldiers `+s`/`+S` for the double step; no castling.
const XIONGQI_START: &str = "rnbeg^bnr/+s+s+s+s+s+s+s+s/8/8/8/8/+S+S+S+S+S+S+S+S/RNBEG^BNR / C/c";

/// Multi-variant game: the first player plays chess (ranks 1-2), the second ōgi
/// (ranks 7-8), on the same board. Style-turn `W/j` (first chess to move, second
/// ōgi).
const MIXED_START: &str = "rnbik^bnr/+f+f+f+f+f+f+f+f/8/8/8/8/+P+P+P+P+P+P+P+P/-RNBQK^BN-R / W/j";

/// A time control generous enough never to flag: the canonical FEEN produced by a
/// move depends only on the rule system, not on the clock.
fn neutral_time_control() -> TimeControl {
    let period = Period::new(Duration::from_secs(3_600), None, None).expect("valid period");
    TimeControl::new(period, Vec::new())
}

/// The canonical FEEN reached by playing `move_src` in `feen`.
fn canonical_after(feen: &str, move_src: &str) -> String {
    let position = Position::parse(feen).expect("valid starting FEEN");
    let state = SessionState::start(position, neutral_time_control(), Timestamp::from_unix(0));
    let half_move = Move::parse(move_src).expect("valid move content");
    match step(state, &half_move, Timestamp::from_unix(0)) {
        StepResult::Advanced { outcome, .. } => outcome.position,
        StepResult::Illegal { reason, .. } => panic!("golden move rejected: {reason}"),
    }
}

#[test]
fn initial_positions_round_trip() {
    for feen in [CHESS_START, OGI_START, XIONGQI_START, MIXED_START] {
        let position = Position::parse(feen).expect("valid initial position");
        assert_eq!(
            position.to_feen(),
            feen,
            "the initial position must be canonical and round-trip stable"
        );
    }
}

#[test]
fn marker_canonicalization_after_a_move() {
    // (label, starting feen, move, expected canonical FEEN)
    let cases = [
        (
            "check marker set",
            "k^7/8/8/8/8/8/8/R6K^ / W/w",
            "[\"a1\",\"a7\",null]",
            "-k^7/R7/8/8/8/8/8/7K^ / w/W",
        ),
        (
            "check marker cleared",
            "-k^7/8/8/8/8/8/8/R6K^ / w/W",
            "[\"a8\",\"b8\",null]",
            "1k^6/8/8/8/8/8/8/R6K^ / W/w",
        ),
        (
            "double-step prefix stripped (single step)",
            "4k^3/8/8/8/8/8/+P7/4K^3 / W/w",
            "[\"a2\",\"a3\",null]",
            "4k^3/8/8/8/8/P7/8/4K^3 / w/W",
        ),
        (
            "en passant marker set",
            "4k^3/8/8/8/p7/8/1+P6/4K^3 / W/w",
            "[\"b2\",\"b4\",null]",
            "4k^3/8/8/8/p-P6/8/8/4K^3 / w/W",
        ),
        (
            "double step with no taker: no en passant marker",
            "4k^3/8/8/8/8/8/+P7/4K^3 / W/w",
            "[\"a2\",\"a4\",null]",
            "4k^3/8/8/8/P7/8/8/4K^3 / w/W",
        ),
        (
            "castling rights removed after a King move",
            "4k^3/8/8/8/8/8/8/+R3K^2+R / W/w",
            "[\"e1\",\"e2\",null]",
            "4k^3/8/8/8/8/8/4K^3/R6R / w/W",
        ),
    ];

    for (label, feen, move_src, expected) in cases {
        assert_eq!(
            canonical_after(feen, move_src),
            expected,
            "case \"{label}\""
        );
    }
}

#[test]
fn markers_preserved_by_the_round_trip() {
    // Each canonical FEEN carrying a marker survives `parse -> to_feen`.
    for feen in [
        "-k^7/R7/8/8/8/8/8/7K^ / w/W",    // check: -k^
        "4k^3/8/8/8/p-P6/8/8/4K^3 / w/W", // en passant: -P
        "4k^3/8/8/8/8/8/4K^3/R6R / w/W",  // castling rights lost: R
    ] {
        let position = Position::parse(feen).expect("valid FEEN");
        assert_eq!(position.to_feen(), feen);
    }
}

#[test]
fn invalid_feens_rejected() {
    for bad in [
        "4k^3/44/8/8/8/8/8/4K^3 / W/w", // non-minimal empty-square count
        "4k^4/8/8/8/8/8/8/4K^3 / W/w",  // rank of 9 squares
        "4k^3/8/8/8/8/8/8/4K^3 W/w",    // missing "hands" field
        "4k^3/8/8/8/8/8/8/4K^3 /",      // missing style-turn
        "4k^3/8/8/8/8/8/8 / W/w",       // 7×8 board (not Sanki)
    ] {
        assert!(
            Position::parse(bad).is_err(),
            "an invalid FEEN was accepted: {bad}"
        );
    }
}
