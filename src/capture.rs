//! Capture transformation — `capture_transform`.
//!
//! When a piece is captured, its EPIN token undergoes a deterministic
//! transformation before entering the **capturer**'s hand. This function is the
//! single source of truth for that rule, **common to all three variants**: there
//! is no per-matchup code. It covers the nine ordered pairs (capturer's variant ×
//! captured piece's variant).
//!
//! The model reduces to two steps (cf. the "Capture Transformation Algorithm" /
//! "All capture cases" sections of `interactions-chess-ogi.md`,
//! `interactions-ogi-xiongqi.md` and `interactions-chess-xiongqi.md`, as well as
//! the "Captures and Pieces in Hand" sections of the per-variant rules):
//!
//! 1. **Base form**: strip the transient state markers (`+`/`-`) and demote the
//!    ōgi Tokin (`T`/`t` -> `F`/`f`), case preserved.
//! 2. **Conversion according to the capturer**:
//!    - **chess** or **xiongqi** capturer: identity (the opponent's case is kept,
//!      hence an *inert* hand by the drop rule);
//!    - **ōgi** capturer taking an **ōgi** piece: case flip (toward the
//!      capturer's side);
//!    - **ōgi** capturer taking a **chess/xiongqi** piece: wholesale conversion
//!      to an **ōgi Fu** in the capturer's side (original type ignored).
//!
//! A held piece's "droppable" character is not encoded here: it follows from the
//! universal rule "droppable iff the case matches the holder's side". Royals are
//! never captured (check decides), so `captured` is never a terminal piece (`^`).

use sashite_epin::sashite_pin::Identifier as Pin;
use sashite_epin::{Identifier as Epin, Letter, Side as PieceSide, State};

use crate::domain::side::Side;
use crate::domain::variant::{Variant, VariantAssignment};

/// Transforms a captured piece into its canonical token in the capturer's hand,
/// according to each side's variant.
///
/// `captured` is the EPIN token of the taken piece (only the opponent is
/// captured: the capturer is the side opposite to `captured`'s).
#[must_use]
pub fn capture_transform(captured: Epin, variants: VariantAssignment) -> Epin {
    let captured_side: Side = captured.side().into();
    let capturer_side = captured_side.flip();
    let captured_variant = variants.variant_of(captured_side);
    let capturer_variant = variants.variant_of(capturer_side);

    let base = to_base(captured);

    match capturer_variant {
        // Chess and Xiongqi do not convert: the base form keeps the (opponent's)
        // case -> the piece is inert in the capturer's hand.
        Variant::Chess | Variant::Xiongqi => base,
        Variant::Ogi => match captured_variant {
            // Intra-ōgi capture: flip the case toward the capturer's side.
            Variant::Ogi => flip_side(base),
            // Capture of a chess/xiongqi piece: wholesale conversion to an ōgi Fu.
            // (The `unwrap_or` is unreachable: `'F'`/`'f'` are always-valid PIN
            // letters — see the tests.)
            Variant::Chess | Variant::Xiongqi => ogi_fu(capturer_side).unwrap_or(base),
        },
    }
}

/// Step 1 — reduction to the base form: stripping the state markers (`+`/`-`)
/// and demoting the ōgi Tokin (`T`/`t` -> `F`/`f`). Case and derivation flag
/// preserved; the Tokin never being terminal, the resulting form isn't either.
#[inline]
fn to_base(captured: Epin) -> Epin {
    let core = captured.pin().normalized(); // strips +/- (state -> Normal)
    let core = match core.letter().as_char() {
        'T' | 't' => demote_tokin(core).unwrap_or(core), // unreachable fallback
        _ => core,
    };
    captured.with_pin(core)
}

/// Demotes a Tokin (`T`/`t`) to a Fu (`F`/`f`), case preserved.
#[inline]
fn demote_tokin(core: Pin) -> Option<Pin> {
    let ch = match core.side() {
        PieceSide::First => 'F',
        PieceSide::Second => 'f',
    };
    let letter = Letter::try_from_char(ch).ok()?;
    Some(Pin::new(letter, core.side(), State::Normal, false))
}

/// Flips a token's case (side), derivation flag preserved.
#[inline]
fn flip_side(piece: Epin) -> Epin {
    piece.with_pin(piece.pin().flipped())
}

/// A native ōgi Fu, unpromoted and unmarked, in side `side`.
#[inline]
fn ogi_fu(side: Side) -> Option<Epin> {
    let piece_side: PieceSide = side.into();
    let ch = match side {
        Side::First => 'F',
        Side::Second => 'f',
    };
    let letter = Letter::try_from_char(ch).ok()?;
    Some(Epin::new(
        Pin::new(letter, piece_side, State::Normal, false),
        false,
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::capture_transform;
    use crate::domain::variant::Variant::{Chess, Ogi, Xiongqi};
    use crate::domain::variant::VariantAssignment;
    use sashite_epin::Identifier as Epin;

    /// Checks that a captured piece yields the expected token under the variant
    /// assignment `va`.
    fn check(captured: &str, va: VariantAssignment, expected: &str) {
        let piece = Epin::parse(captured).expect("valid captured EPIN token");
        let got = capture_transform(piece, va);
        assert_eq!(
            got.encode().as_str(),
            expected,
            "capture of `{captured}` under {va:?} expected `{expected}`",
        );
    }

    // --- Chess vs Ōgi (interactions-chess-ogi.md, "All Capture Cases") ---
    // J/w context: first = ōgi (uppercase), second = chess (lowercase).
    #[test]
    fn chess_ogi_jw() {
        let va = VariantAssignment::new(Ogi, Chess);
        // Case 1: chess takes the ōgi Tokin -> inert Fu (ōgi case kept).
        check("T", va, "F");
        // Case 2: chess takes a non-Tokin ōgi piece -> base form, inert.
        check("+F", va, "F");
        check("I", va, "I");
        check("R", va, "R");
        check("B", va, "B");
        check("N", va, "N");
        // Case 3: ōgi takes any chess piece -> ōgi Fu (droppable).
        check("q", va, "F");
        check("+p", va, "F");
        check("-p", va, "F");
        check("+r", va, "F");
        check("-r", va, "F");
        check("n", va, "F");
        check("b", va, "F");
    }

    // Mirror context W/j: first = chess, second = ōgi.
    #[test]
    fn chess_ogi_wj() {
        let va = VariantAssignment::new(Chess, Ogi);
        check("t", va, "f"); // chess takes ōgi Tokin -> lowercase inert Fu
        check("+f", va, "f");
        check("i", va, "i");
        check("Q", va, "f"); // ōgi takes chess piece -> lowercase ōgi Fu (droppable)
        check("+P", va, "f");
        check("-R", va, "f");
    }

    // --- Ōgi vs Xiongqi (interactions-ogi-xiongqi.md) ---
    // J/c context: first = ōgi (uppercase), second = xiongqi (lowercase).
    #[test]
    fn ogi_xiongqi_jc() {
        let va = VariantAssignment::new(Ogi, Xiongqi);
        // Xiongqi takes an ōgi piece -> identity (Tokin demoted), inert.
        check("T", va, "F");
        check("+F", va, "F");
        check("-F", va, "F");
        check("I", va, "I");
        check("R", va, "R");
        check("B", va, "B");
        check("N", va, "N");
        // Ōgi takes a xiongqi piece -> ōgi Fu (droppable).
        check("s", va, "F");
        check("+s", va, "F");
        check("-s", va, "F");
        check("r", va, "F");
        check("n", va, "F");
        check("b", va, "F");
        check("e", va, "F");
    }

    // --- Chess vs Xiongqi (interactions-chess-xiongqi.md) ---
    // Identity minus markers in both directions; no droppable hand.
    // C/w context via new(Chess, Xiongqi): first = chess, second = xiongqi.
    #[test]
    fn chess_xiongqi() {
        let va = VariantAssignment::new(Chess, Xiongqi);
        // Chess takes a xiongqi piece -> identity, case kept, inert.
        check("+s", va, "s");
        check("-s", va, "s");
        check("e", va, "e");
        check("r", va, "r");
        check("n", va, "n");
        check("b", va, "b");
        // Xiongqi takes a chess piece -> identity, case kept, inert.
        check("+P", va, "P");
        check("-P", va, "P");
        check("Q", va, "Q");
        check("+R", va, "R");
        check("N", va, "N");
    }

    // --- Pure variants (mirror matchups) ---
    #[test]
    fn pure_ogi() {
        let va = VariantAssignment::new(Ogi, Ogi);
        // Sente (uppercase) takes Gote (lowercase): demotion + flip to uppercase.
        check("f", va, "F");
        check("+f", va, "F");
        check("-f", va, "F");
        check("t", va, "F");
        check("n", va, "N");
        check("r", va, "R");
        check("b", va, "B");
        check("i", va, "I");
        // Reverse direction.
        check("T", va, "f");
        check("N", va, "n");
    }

    #[test]
    fn pure_chess() {
        let va = VariantAssignment::new(Chess, Chess);
        // White takes Black: identity, (lowercase) case kept, inert.
        check("+p", va, "p");
        check("-p", va, "p");
        check("+r", va, "r");
        check("q", va, "q");
        check("n", va, "n");
        // Reverse direction.
        check("+P", va, "P");
    }

    #[test]
    fn pure_xiongqi() {
        let va = VariantAssignment::new(Xiongqi, Xiongqi);
        // No demotion: the Empress stays an Empress, etc.
        check("+s", va, "s");
        check("-s", va, "s");
        check("e", va, "e");
        check("r", va, "r");
        check("n", va, "n");
        check("b", va, "b");
        check("+E", va, "E"); // (E has no state in practice; we check the normalization)
    }
}
