//! Per-variant and cross-variant perft: node counts (regression).
//!
//! This file's single responsibility: a **perft** — counting the leaves of the
//! legal-move tree down to a given depth — built on the **public façade**
//! [`engine`](sashite_sanki_engine::engine) (`legal_moves` + `apply`), checked against
//! reference counts. Resting on the public API makes it a test of that API too.
//!
//! # Why it is conclusive
//!
//! The **chess** perft from the standard initial position must reproduce the
//! universally known **FIDE** numbers (20, 400, 8902, 197281, …): an **external
//! truth**. If the generator diverges by a single move (legality, pins, castling,
//! en passant, promotion), the count changes.
//!
//! For **ōgi** and **xiongqi** (non-standard 8×8 variants), no reference perft is
//! published: the counts are **frozen from this implementation** and serve as a
//! **regression** guard. Confidence comes by transitivity: the generator's core,
//! validated exactly on chess, is shared. (The ōgi perft at depth 4 also exercises
//! drops: a Fu captured on the 3rd ply may be dropped on the 4th.)
//!
//! `MIXED_START` (the same chess/ōgi FEEN frozen in `golden_feen.rs`) gets the
//! same regression treatment, for the same reason: no reference perft exists for
//! a cross-variant board either. It closes a real gap — every count above comes
//! from a pure pairing, so none of them ever exercises the cross-variant
//! capture-transformation model (`crate::capture`) that only fires when the two
//! sides run different rule systems. Its ōgi (second) side is the only one that
//! can ever hold a droppable piece here, and a capture-then-drop sequence takes
//! longer to arise on this board than in pure ōgi (confirmed absent through depth
//! 5); the entries below stop at depth 4, already deep enough to be a meaningful
//! regression guard.
//!
//! The heavy depths (≈ 30 s per starting position in a debug build) are grouped
//! in an `#[ignore]`d test, run on demand.

#![allow(clippy::expect_used, clippy::panic, clippy::arithmetic_side_effects)]

use sashite_sanki_engine::engine::{apply, legal_moves};
use sashite_sanki_engine::position::Position;

const CHESS_START: &str = "-rnbqk^bn-r/+p+p+p+p+p+p+p+p/8/8/8/8/+P+P+P+P+P+P+P+P/-RNBQK^BN-R / W/w";
const OGI_START: &str = "-rnbik^bn-r/+f+f+f+f+f+f+f+f/8/8/8/8/+F+F+F+F+F+F+F+F/-RNBIK^BN-R / J/j";
const XIONGQI_START: &str =
    "-rnbeg^bn-r/+s+s+s+s+s+s+s+s/8/8/8/8/+S+S+S+S+S+S+S+S/-RNBEG^BN-R / C/c";
// Chess (first) vs ōgi (second) on the shared board — same value as
// `golden_feen.rs`'s `MIXED_START`, reused here for consistency.
const MIXED_START: &str = "-rnbik^bn-r/+f+f+f+f+f+f+f+f/8/8/8/8/+P+P+P+P+P+P+P+P/-RNBQK^BN-R / W/j";

/// The number of legal-move leaves at `depth` plies from `position`.
fn perft(position: &Position, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let mut nodes = 0;
    for mv in legal_moves(position) {
        let next = apply(position, &mv).expect("legal move by construction");
        nodes += perft(&next, depth - 1);
    }
    nodes
}

fn start(feen: &str) -> Position {
    Position::parse(feen).expect("valid initial position")
}

#[test]
fn perft_chess_fide_numbers() {
    // FIDE reference from the standard initial position — external truth.
    let chess = start(CHESS_START);
    assert_eq!(perft(&chess, 1), 20);
    assert_eq!(perft(&chess, 2), 400);
    assert_eq!(perft(&chess, 3), 8902);
}

#[test]
fn perft_ogi_regression() {
    let ogi = start(OGI_START);
    assert_eq!(perft(&ogi, 1), 22);
    assert_eq!(perft(&ogi, 2), 484);
}

#[test]
fn perft_xiongqi_regression() {
    let xiongqi = start(XIONGQI_START);
    assert_eq!(perft(&xiongqi, 1), 22);
    assert_eq!(perft(&xiongqi, 2), 484);
}

#[test]
fn perft_mixed_regression() {
    // The one pairing above that is not a pure variant on both sides: no
    // reference perft exists for it either, frozen from this implementation
    // exactly as the ōgi/xiongqi regressions above.
    let mixed = start(MIXED_START);
    assert_eq!(perft(&mixed, 1), 20);
    assert_eq!(perft(&mixed, 2), 440);
}

#[test]
#[ignore = "deep perft (≈ 30 s per starting position in a debug build) — on demand"]
fn perft_deep() {
    assert_eq!(perft(&start(CHESS_START), 4), 197_281);
    assert_eq!(perft(&start(OGI_START), 3), 11_936);
    assert_eq!(perft(&start(OGI_START), 4), 291_782);
    assert_eq!(perft(&start(XIONGQI_START), 3), 11_801);
    assert_eq!(perft(&start(XIONGQI_START), 4), 285_767);
    assert_eq!(perft(&start(MIXED_START), 3), 9_792);
    assert_eq!(perft(&start(MIXED_START), 4), 240_261);
}
