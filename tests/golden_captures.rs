//! Golden vectors: the "All capture cases" tables from the three interaction docs.
//!
//! This file's single responsibility: to validate the **capture transformation**
//! end-to-end — not the pure EPIN function (already covered by `crate::capture`'s
//! unit tests), but its observable effect during a real capture played by the
//! kernel. Each case plays a capturing move via [`step`] and checks the
//! **resulting FEEN**, in particular the "hands" field: the captured piece must
//! enter it in its transformed canonical form.
//!
//! The cases mirror, one by one, the rows of the "All capture cases" tables of
//! `interactions-chess-ogi.md`, `interactions-ogi-xiongqi.md` and
//! `interactions-chess-xiongqi.md`, rounded out by the three pure-variant
//! captures. The rule in brief:
//!
//! - a **chess** or **xiongqi** capturer preserves the identity (type and side)
//!   of the taken piece — only the `+`/`-` markers drop and the ōgi Tokin is
//!   demoted to a Fu; the resulting hand is *inert* (wrong case);
//! - an **ōgi** capturer converts any non-ōgi piece into a **Fu of its own side**
//!   (droppable), and flips the side of an ōgi piece.
//!
//! Common setup: the capturer's Rook/Chariot (first player) is on `a1`, the
//! captured piece (second player) on `a5`; the move is always the capture
//! `a1 x a5`. The opposing king is placed on `h8` — not on the `e`-file — to avoid
//! a **xiongqi General**, which attacks like a Chariot, putting the capturer's
//! king in check along the shared file (which would make the capture illegal).
//! This file is **self-contained**.

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
use sashite_sanki_engine::kernel::step::step;
use sashite_sanki_engine::position::Position;

/// The capturing move shared by every case: the rook/chariot on `a1` takes the
/// piece on `a5`.
const CAPTURE: &str = "[\"a1\",\"a5\",null]";

/// A time control generous enough never to flag.
fn neutral_time_control() -> TimeControl {
    let period = Period::new(Duration::from_secs(3_600), None, None).expect("valid period");
    TimeControl::new(period, Vec::new())
}

/// The canonical FEEN reached by playing the capture in `feen`.
fn canonical_after(feen: &str) -> String {
    let position = Position::parse(feen).expect("valid starting FEEN");
    let state = SessionState::start(position, neutral_time_control(), Timestamp::from_unix(0));
    let half_move = Move::parse(CAPTURE).expect("valid capture move");
    step(state, &half_move, Timestamp::from_unix(0))
        .outcome
        .position
}

#[test]
fn captures_transform_the_piece_in_hand() {
    // (label / table reference, starting feen, expected canonical FEEN)
    //
    // The "hands" field of the expected FEEN (right after the placement) carries
    // the transformed captured piece: `r/`, `q/`, `f/` (identity, inert) for a
    // chess/xiongqi capturer; `F/` (droppable Fu) for an ōgi capturer; `R/` (side
    // flipped) for pure ōgi.
    let cases = [
        // --- Chess vs Ōgi (interactions-chess-ogi.md, "All Capture Cases") ---
        // Case 2: chess takes a non-Tokin ōgi piece -> identity, inert.
        (
            "chess <- ogi (case 2)",
            "7k^/8/8/r7/8/8/8/R3K^3 / W/j",
            "7k^/8/8/R7/8/8/8/4K^3 r/ j/W",
        ),
        // Case 1: chess takes an ōgi Tokin -> demoted Fu, inert.
        (
            "chess <- ōgi tokin (case 1)",
            "7k^/8/8/t7/8/8/8/R3K^3 / W/j",
            "7k^/8/8/R7/8/8/8/4K^3 f/ j/W",
        ),
        // Case 3: ōgi takes a chess piece -> ōgi Fu of the ōgi side, droppable.
        (
            "ōgi <- chess (case 3)",
            "7k^/8/8/r7/8/8/8/R3K^3 / J/w",
            "7k^/8/8/R7/8/8/8/4K^3 F/ w/J",
        ),
        // --- Ōgi vs Xiongqi (interactions-ogi-xiongqi.md) ---
        // Case 1: xiongqi takes an ōgi Tokin -> demoted Fu, inert.
        (
            "xiongqi <- ōgi tokin (case 1)",
            "7k^/8/8/t7/8/8/8/R3G^3 / C/j",
            "7k^/8/8/R7/8/8/8/4G^3 f/ j/C",
        ),
        // Case 3: ōgi takes a xiongqi piece -> ōgi Fu, droppable.
        (
            "ōgi <- xiongqi (case 3)",
            "7g^/8/8/e7/8/8/8/R3K^3 / J/c",
            "7g^/8/8/R7/8/8/8/4K^3 F/ c/J",
        ),
        // --- Chess vs Xiongqi (interactions-chess-xiongqi.md) ---
        // Case 1: chess takes a xiongqi piece -> identity, inert.
        (
            "chess <- xiongqi (case 1)",
            "7g^/8/8/r7/8/8/8/R3K^3 / W/c",
            "7g^/8/8/R7/8/8/8/4K^3 r/ c/W",
        ),
        // Case 2: xiongqi takes a chess piece -> identity, inert.
        (
            "xiongqi <- chess (case 2)",
            "7k^/8/8/q7/8/8/8/R3G^3 / C/w",
            "7k^/8/8/R7/8/8/8/4G^3 q/ w/C",
        ),
        // --- Pure variants ---
        // Pure chess: identity, opponent's case kept, inert.
        (
            "pure chess",
            "7k^/8/8/r7/8/8/8/R3K^3 / W/w",
            "7k^/8/8/R7/8/8/8/4K^3 r/ w/W",
        ),
        // Pure ōgi: the case flips to the capturer's side -> droppable.
        (
            "pure ōgi",
            "7k^/8/8/r7/8/8/8/R3K^3 / J/j",
            "7k^/8/8/R7/8/8/8/4K^3 R/ j/J",
        ),
        // Pure xiongqi: identity, opponent's case kept, inert.
        (
            "pure xiongqi",
            "7g^/8/8/r7/8/8/8/R3G^3 / C/c",
            "7g^/8/8/R7/8/8/8/4G^3 r/ c/C",
        ),
    ];

    for (label, feen, expected) in cases {
        assert_eq!(canonical_after(feen), expected, "case \"{label}\"");
    }
}
