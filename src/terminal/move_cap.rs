//! The **absolute move cap** (Sanki global rule): every game is bounded to
//! **300 full moves — 600 half-moves**. If the game is still ongoing once 600
//! half-moves have been played, it is **automatically** a draw (`movecap`),
//! whatever the position on the board.
//!
//! Unlike the 50-move rule ([`super::move_limit`]), this ceiling **never
//! resets**; it guarantees every Sanki game terminates within a bounded number
//! of half-moves, regardless of variant or pairing. See
//! `web-specs.md/rules/rules/sanki-global-rules.md`.
//!
//! The total half-move count is maintained by the kernel (the play-order
//! position of the next ply, minus one); this module owns only the constant and
//! the threshold test.

/// Number of half-moves (300 full moves) at which the game is automatically a
/// draw when still ongoing.
pub const HALF_MOVE_CAP: u32 = 600;

/// True once at least [`HALF_MOVE_CAP`] half-moves have been played — the
/// `movecap` draw applies when the position is otherwise still ongoing.
#[inline]
#[must_use]
pub const fn cap_reached(half_moves_played: u32) -> bool {
    half_moves_played >= HALF_MOVE_CAP
}

#[cfg(test)]
mod tests {
    use super::{cap_reached, HALF_MOVE_CAP};

    #[test]
    fn threshold() {
        assert_eq!(HALF_MOVE_CAP, 600);
        assert!(!cap_reached(599));
        assert!(cap_reached(HALF_MOVE_CAP));
        assert!(cap_reached(601));
    }
}
