//! `Piece` — a thin semantic surface over `epin::Identifier`.
//!
//! A `Copy` newtype (the EPIN identifier is 5 bytes) exposing the
//! *variant-independent* predicates that can be read straight from the token:
//! side (case), the "royal" marker (`^`), transient state (`+`/`-`), the type
//! letter, and membership in the foot-soldier class (Pawn / Fu / Soldier).
//!
//! Variant-dependent classification — slider, leaper, foot soldier *of which
//! variant* — belongs to [`crate::movement`], which dispatches on the
//! (variant, type letter) pair. Piece transformations (capture, promotion)
//! operate directly on the underlying `epin::Identifier`.

use crate::domain::side::Side;
use sashite_epin::sashite_pin::Identifier as Pin;
use sashite_epin::{Identifier as Epin, Letter, Side as PieceSide, State};

/// A piece, viewed as an EPIN token enriched with domain predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Piece(Epin);

impl Piece {
    /// Wraps an EPIN token.
    #[inline]
    #[must_use]
    pub const fn new(epin: Epin) -> Self {
        Self(epin)
    }

    /// The underlying EPIN token (for transformations: capture, promotion).
    #[inline]
    #[must_use]
    pub const fn epin(self) -> Epin {
        self.0
    }

    /// The piece's side (carried by the token's case).
    #[inline]
    #[must_use]
    pub fn side(self) -> Side {
        self.0.side().into()
    }

    /// True if the piece is **royal** (its capture ends the game) — terminal
    /// marker `^`. In Sanki: the King (`K^`) and the General (`G^`).
    #[inline]
    #[must_use]
    pub fn is_royal(self) -> bool {
        self.0.is_terminal()
    }

    /// The piece's **type** letter, uppercased (case-insensitive identity).
    /// Used by the `(variant, letter)` dispatch in [`crate::movement`].
    #[inline]
    #[must_use]
    pub fn kind_letter(self) -> char {
        self.0.letter().as_char().to_ascii_uppercase()
    }

    /// True if the piece is a **foot soldier** (the Pawn / Fu / Soldier class) —
    /// the letters `P` / `F` / `S`, each exclusive to its variant. Notably used
    /// by the 50-move rule.
    #[inline]
    #[must_use]
    pub fn is_foot_soldier(self) -> bool {
        matches!(self.kind_letter(), 'P' | 'F' | 'S')
    }

    /// *Enhanced* state (`+`) — e.g. double-step eligibility, castling right.
    #[inline]
    #[must_use]
    pub fn is_enhanced(self) -> bool {
        matches!(self.0.state(), State::Enhanced)
    }

    /// *Diminished* state (`-`) — e.g. en-passant vulnerability, blocked
    /// castling, royal in check.
    #[inline]
    #[must_use]
    pub fn is_diminished(self) -> bool {
        matches!(self.0.state(), State::Diminished)
    }

    /// Normal state (no `+`/`-` marker).
    #[inline]
    #[must_use]
    pub fn is_normal(self) -> bool {
        matches!(self.0.state(), State::Normal)
    }

    /// True if the piece belongs to `side`.
    #[inline]
    #[must_use]
    pub fn belongs_to(self, side: Side) -> bool {
        self.side() == side
    }

    /// The same piece **promoted** to the base letter `letter` (e.g. `'Q'`,
    /// `'E'`, `'T'`): the type changes, but the **side** and the **derivation
    /// status** (native/derived) are preserved, the state resets to normal, and
    /// the piece becomes non-terminal (a promoted piece is never royal). Returns
    /// `None` if `letter` is not a valid letter.
    ///
    /// The single source of truth for promotion's type change — mirrors the
    /// transformations in [`crate::capture`] via `with_pin`.
    #[must_use]
    pub fn promoted_to(self, letter: char) -> Option<Self> {
        let side = self.0.side();
        let cased = match side {
            PieceSide::First => letter.to_ascii_uppercase(),
            PieceSide::Second => letter.to_ascii_lowercase(),
        };
        let l = Letter::try_from_char(cased).ok()?;
        let pin = Pin::new(l, side, State::Normal, false);
        Some(Self(self.0.with_pin(pin)))
    }
}

impl From<Epin> for Piece {
    #[inline]
    fn from(epin: Epin) -> Self {
        Self(epin)
    }
}

impl From<Piece> for Epin {
    #[inline]
    fn from(piece: Piece) -> Self {
        piece.0
    }
}

impl core::fmt::Display for Piece {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.0.encode().as_str())
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::Piece;
    use crate::domain::side::Side;
    use sashite_epin::Identifier as Epin;

    fn piece(token: &str) -> Piece {
        Piece::new(Epin::parse(token).expect("valid EPIN token"))
    }

    #[test]
    fn royals() {
        assert!(piece("K^").is_royal()); // chess/ōgi king
        assert!(piece("g^").is_royal()); // xiongqi general
        assert!(!piece("R").is_royal());
        assert!(!piece("q").is_royal());
    }

    #[test]
    fn type_letter_is_uppercase() {
        assert_eq!(piece("K^").kind_letter(), 'K');
        assert_eq!(piece("g^").kind_letter(), 'G');
        assert_eq!(piece("p").kind_letter(), 'P');
        assert_eq!(piece("-s").kind_letter(), 'S');
        assert_eq!(piece("t").kind_letter(), 'T');
        assert_eq!(piece("I").kind_letter(), 'I');
        assert_eq!(piece("e").kind_letter(), 'E');
    }

    #[test]
    fn foot_soldiers() {
        assert!(piece("P").is_foot_soldier());
        assert!(piece("-P").is_foot_soldier());
        assert!(piece("F").is_foot_soldier());
        assert!(piece("f").is_foot_soldier());
        assert!(piece("S").is_foot_soldier());
        assert!(piece("-s").is_foot_soldier());
        // The Tokin (promoted Fu) is no longer a foot soldier.
        assert!(!piece("t").is_foot_soldier());
        // Other pieces.
        assert!(!piece("R").is_foot_soldier());
        assert!(!piece("I").is_foot_soldier());
        assert!(!piece("E").is_foot_soldier());
        assert!(!piece("K^").is_foot_soldier());
    }

    #[test]
    fn states() {
        assert!(piece("+R").is_enhanced()); // rook: castling available
        assert!(!piece("+R").is_normal());
        assert!(piece("-P").is_diminished()); // pawn: en-passant vulnerable
        assert!(piece("P").is_normal());
        assert!(!piece("P").is_enhanced());
    }

    #[test]
    fn side_and_membership() {
        assert_eq!(piece("R").side(), Side::First); // uppercase -> first
        assert_eq!(piece("r").side(), Side::Second); // lowercase -> second
        assert!(piece("R").belongs_to(Side::First));
        assert!(!piece("R").belongs_to(Side::Second));
    }

    #[test]
    fn conversions_and_display() {
        let e = Epin::parse("+K^").expect("valid");
        let p = Piece::from(e);
        assert_eq!(Epin::from(p), e);
        assert_eq!(p.epin(), e);
        assert_eq!(p.to_string(), "+K^");
    }
}
