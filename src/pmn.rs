//! Canonical **PMN** of Sanki moves — the notation the Sashité Engine Interface
//! (SEI) carries — and its correspondence with the Ply content
//! `[source, destination, actor]`, as fixed by *SEI Rules Document — Sanki*
//! (`sashite.sanki.kernel/1`, `web-specs.md/rules/sei-sanki.md`).
//!
//! Every legal move has exactly one PMN string:
//!
//! | Effect | PMN | Example |
//! |---|---|---|
//! | move to an empty square | `src-dst` | `e2-e4` |
//! | ordinary capture | `src+dst[/captured]` | `e4+d5`, `e1+d1/F` |
//! | castling | `src~dst` (the royal's squares) | `e1~g1` |
//! | en passant | `src~dst[/captured]` (the capturer's squares) | `b5~a6` |
//! | promotion | `src-dst=actor`, `src+dst=actor[/captured]` | `a7-a8=Q`, `b2+b1=t/f` |
//! | drop | `piece*dst` | `F*e4`, `f*f7` |
//!
//! - `~` is written for a castling and for an en passant capture, and for
//!   nothing else; `+` for a capture on the destination square; `-` otherwise.
//! - `=actor` is present if and only if the move promotes: the target letter,
//!   in the mover's case, without marker — `=T` for a fu although the mover has
//!   no choice. State markers the mover loses or gains are recompositions of the
//!   position (Kernel — Sanki §I.7) and are never written.
//! - `/captured` is present if and only if the move captures and the token that
//!   enters the capturer's hand differs from the captured piece's bare letter
//!   (case included): always for an ōgi capturer; for a chess or xiongqi
//!   capturer, only when the victim is a tokin, demoted to `f`.
//! - A drop names the dropped piece: the hand token, bare, in the dropper's
//!   case. A drop without its piece is **malformed** for SEI (`invalid`).
//!
//! [`to_pmn`] derives the string of a legal move from the position, through the
//! legality layer's [`Effect`]; [`from_pmn`] reads the content back, structurally;
//! [`parse_canonical`] does what an SEI host or engine needs: reads, validates,
//! and refuses a well-formed string that is not *the* canonical one — membership
//! in the legal set is not canonicity (`e1-g1`, `a7-a8` without `=T`, `e4-d5`
//! for a capture all map to legal contents).

use crate::apply::Effect;
use crate::capture::capture_transform;
use crate::domain::actor::ActorName;
use crate::domain::half_move::Move;
use crate::domain::outcome::IllegalReason;
use crate::domain::piece::Piece;
use crate::domain::square::Square;
use crate::domain::variant::Variant;
use crate::legality::resolve::resolve;
use crate::position::Position;
use core::fmt::{self, Write as _};
use sashite_epin::Identifier as Epin;

/// Why a PMN string could not be read as a Sanki move, in SEI's terms: the first
/// three variants are `invalid` (the string is not a PMN the rules document can
/// carry), the others `illegal` (a well-formed string that names no legal,
/// canonical move of the position).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PmnError {
    /// Not a PMN string at all, or a form that cannot be spelled on an 8×8
    /// board (SEI: `invalid`).
    Malformed,
    /// A drop written without its piece, `*e4` (SEI: `invalid`; the rules
    /// document requires the piece).
    DropWithoutPiece,
    /// A square outside the 8×8 board (SEI: `illegal`, the geometry is the
    /// rules').
    OffBoard,
    /// A PMN form that no Sanki move takes: the Pass Move, a static capture, an
    /// in-place mutation, a drop with capture (SEI: `illegal`).
    UnsupportedForm,
    /// A dropped or promotion piece that the mover's variant does not name, or
    /// a token no hand holds (a marked or derived drop piece) (SEI: `illegal`).
    UnknownPiece,
    /// The move the string names is not legal in the position (SEI: `illegal`).
    Illegal(IllegalReason),
    /// The move is legal, but the string is not its canonical PMN (SEI:
    /// `illegal`); `canonical` is the one string the rules document writes.
    NotCanonical {
        /// The canonical PMN of the move the string names.
        canonical: String,
    },
}

impl PmnError {
    /// True for the errors SEI reports as `invalid` (a malformed request), false
    /// for those it reports as `illegal`.
    #[must_use]
    pub const fn is_invalid(&self) -> bool {
        matches!(self, Self::Malformed | Self::DropWithoutPiece)
    }
}

impl fmt::Display for PmnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => f.write_str("not a PMN move"),
            Self::DropWithoutPiece => f.write_str("a drop must name its piece"),
            Self::OffBoard => f.write_str("a square is outside the 8x8 board"),
            Self::UnsupportedForm => f.write_str("a PMN form no Sanki move takes"),
            Self::UnknownPiece => f.write_str("a piece the mover's variant does not name"),
            Self::Illegal(reason) => write!(f, "illegal move: {reason:?}"),
            Self::NotCanonical { canonical } => {
                write!(f, "not the canonical PMN of the move ({canonical})")
            }
        }
    }
}

impl core::error::Error for PmnError {}

/// The canonical PMN of `mv` in `position`.
///
/// # Errors
/// The [`IllegalReason`] of the legality layer when `mv` is not legal in
/// `position`: only a legal move has a canonical PMN.
pub fn to_pmn(position: &Position, mv: &Move) -> Result<String, IllegalReason> {
    // The full rule system first (uchifuzume included), then the effect.
    crate::engine::validate(position, mv)?;
    let effect = resolve(position, mv)?;
    let mut out = String::with_capacity(12);
    match effect {
        Effect::Drop { piece, to } => {
            // The hand token, exactly as held (bare, in the dropper's case).
            let _ = write!(out, "{piece}*{to}");
        }
        Effect::Castle(castling) => {
            let _ = write!(out, "{}~{}", castling.king_from, castling.king_to);
        }
        Effect::Board {
            from,
            to,
            placed,
            captured,
        } => {
            let mover = position
                .piece_at(from)
                .ok_or(IllegalReason::NoMoverPieceAtSource)?;
            let operator = match captured {
                None => '-',
                Some(square) if square == to => '+',
                Some(_) => '~',
            };
            let _ = write!(out, "{from}{operator}{to}");
            if !same_identity(mover.epin(), placed.epin()) {
                out.push('=');
                out.push(cased_letter(placed.epin()));
            }
            if let Some(square) = captured {
                let victim = position
                    .piece_at(square)
                    .ok_or(IllegalReason::IllegalDestination)?;
                let token = capture_transform(victim.epin(), position.variants());
                if cased_letter(token) != cased_letter(victim.epin()) {
                    out.push('/');
                    let _ = write!(out, "{}", Piece::new(token));
                }
            }
        }
    }
    Ok(out)
}

/// Reads a PMN string as a Ply content, structurally: the squares, the operator's
/// form, the dropped piece's name and the promotion target under the mover's
/// variant. Legality and canonicity are [`parse_canonical`]'s.
///
/// # Errors
/// See [`PmnError`]; this function never returns `Illegal` or `NotCanonical`.
pub fn from_pmn(position: &Position, pmn: &str) -> Result<Move, PmnError> {
    let variant = position.active_variant();
    if pmn == "..." {
        return Err(PmnError::UnsupportedForm);
    }
    if let Some((piece, rest)) = pmn.split_once('*') {
        return read_drop(variant, piece, rest);
    }
    if pmn.contains('.') {
        return Err(PmnError::UnsupportedForm);
    }
    if pmn.starts_with('+') {
        return Err(PmnError::UnsupportedForm);
    }
    read_board(variant, pmn)
}

/// Reads a drop `piece*dst`: `piece` is the part before `*`, `rest` the part after.
fn read_drop(variant: Variant, piece: &str, rest: &str) -> Result<Move, PmnError> {
    if piece.is_empty() {
        return Err(PmnError::DropWithoutPiece);
    }
    if rest.contains('=') {
        // `piece*dst=actor`: a mutating drop, which no Sanki move is.
        return Err(PmnError::UnsupportedForm);
    }
    let to = read_square(rest)?;
    let epin = Epin::parse(piece).map_err(|_| PmnError::Malformed)?;
    if !matches!(epin.state(), sashite_epin::State::Normal)
        || epin.is_terminal()
        || epin.is_derived()
    {
        // A well-formed token that no hand holds.
        return Err(PmnError::UnknownPiece);
    }
    let name = ActorName::for_letter(variant, epin.letter().as_char().to_ascii_uppercase())
        .ok_or(PmnError::UnknownPiece)?;
    Ok(Move::Drop { piece: name, to })
}

/// Reads a board move `src(-|+|~)dst[=actor][/captured]`.
fn read_board(variant: Variant, pmn: &str) -> Result<Move, PmnError> {
    let operator_at = pmn.find(['-', '+', '~']).ok_or(PmnError::Malformed)?;
    let (src, after) = pmn.split_at(operator_at);
    let mut rest = after.chars();
    let _operator = rest.next().ok_or(PmnError::Malformed)?;
    let rest = rest.as_str();
    // The destination ends at `=` or `/`, whichever comes first.
    let dst_end = rest.find(['=', '/']).unwrap_or(rest.len());
    let (dst, suffixes) = rest.split_at(dst_end);
    let from = read_square(src)?;
    let to = read_square(dst)?;
    if from == to {
        return Err(PmnError::Malformed);
    }
    let (actor, _captured) = read_suffixes(suffixes)?;
    let actor = match actor {
        None => None,
        // A promotion target is named in the content only where the variant
        // leaves the choice; whether the move promotes at all is legality's.
        Some(letter) => match variant {
            Variant::Ogi => None,
            Variant::Chess | Variant::Xiongqi => Some(
                ActorName::for_letter(variant, letter.to_ascii_uppercase())
                    .ok_or(PmnError::UnknownPiece)?,
            ),
        },
    };
    Ok(Move::Board { from, to, actor })
}

/// Reads `[=actor][/captured]`, in that order, each an EPIN token; returns the
/// two letters.
fn read_suffixes(suffixes: &str) -> Result<(Option<char>, Option<char>), PmnError> {
    let mut actor = None;
    let mut captured = None;
    let mut rest = suffixes;
    if let Some(after) = rest.strip_prefix('=') {
        let end = after.find('/').unwrap_or(after.len());
        let (token, tail) = after.split_at(end);
        actor = Some(read_token_letter(token)?);
        rest = tail;
    }
    if let Some(token) = rest.strip_prefix('/') {
        captured = Some(read_token_letter(token)?);
        rest = "";
    }
    if !rest.is_empty() {
        return Err(PmnError::Malformed);
    }
    Ok((actor, captured))
}

/// The letter of a suffix token, which must be a valid EPIN identifier.
fn read_token_letter(token: &str) -> Result<char, PmnError> {
    let epin = Epin::parse(token).map_err(|_| PmnError::Malformed)?;
    Ok(cased_letter(epin))
}

/// The token's letter in the case of its side: uppercase for `first`, lowercase
/// for `second`.
fn cased_letter(epin: Epin) -> char {
    let letter = epin.letter().as_char();
    if epin.is_first() {
        letter.to_ascii_uppercase()
    } else {
        letter.to_ascii_lowercase()
    }
}

/// A CELL coordinate, on the 8×8 board.
fn read_square(text: &str) -> Result<Square, PmnError> {
    let bytes = text.as_bytes();
    let files = bytes.iter().take_while(|b| b.is_ascii_lowercase()).count();
    let ranks = bytes
        .iter()
        .skip(files)
        .take_while(|b| b.is_ascii_digit())
        .count();
    let layers = bytes
        .iter()
        .skip(files.saturating_add(ranks))
        .take_while(|b| b.is_ascii_uppercase())
        .count();
    let well_formed = (1..=2).contains(&files)
        && (1..=3).contains(&ranks)
        && layers <= 2
        && files.saturating_add(ranks).saturating_add(layers) == bytes.len()
        && bytes.get(files) != Some(&b'0');
    if !well_formed {
        return Err(PmnError::Malformed);
    }
    Square::parse(text).map_err(|_| PmnError::OffBoard)
}

/// Whether `pmn` is well-formed for SEI, without a position: the checks that
/// yield `invalid` — a string that is not a PMN move at all, a form that
/// cannot be spelled on the board, a drop without its piece. Everything a
/// position decides (`illegal`) is left to [`parse_canonical`]. SEI reports
/// an `invalid` before an `unsupported` or an `illegal`, so a host or an
/// engine runs this over every move first.
///
/// # Errors
/// [`PmnError::Malformed`] or [`PmnError::DropWithoutPiece`], and nothing else.
pub fn well_formed(pmn: &str) -> Result<(), PmnError> {
    if pmn == "..." || pmn.starts_with('+') || pmn.contains('.') {
        return Ok(()); // well-formed PMN forms that no Sanki move takes: `illegal`, later
    }
    if let Some((piece, rest)) = pmn.split_once('*') {
        if piece.is_empty() {
            return Err(PmnError::DropWithoutPiece);
        }
        if rest.contains('=') {
            return Ok(());
        }
        Epin::parse(piece).map_err(|_| PmnError::Malformed)?;
        return read_square(rest).map(|_| ()).or_else(|e| match e {
            PmnError::Malformed => Err(PmnError::Malformed),
            _ => Ok(()),
        });
    }
    let operator_at = pmn.find(['-', '+', '~']).ok_or(PmnError::Malformed)?;
    let (src, after) = pmn.split_at(operator_at);
    let rest = after.get(1..).ok_or(PmnError::Malformed)?;
    let dst_end = rest.find(['=', '/']).unwrap_or(rest.len());
    let (dst, suffixes) = rest.split_at(dst_end);
    for square in [src, dst] {
        if let Err(PmnError::Malformed) = read_square(square) {
            return Err(PmnError::Malformed);
        }
    }
    if src == dst {
        return Err(PmnError::Malformed);
    }
    read_suffixes(suffixes).map(|_| ())
}

/// Reads `pmn` as a legal move of `position` written in its canonical form —
/// what an SEI engine does with every move of `moves`, and an SEI host with the
/// engine's `best` before it plays it.
///
/// # Errors
/// See [`PmnError`]: a malformed string, a string naming no legal move, or a
/// well-formed string that is not the move's canonical PMN.
pub fn parse_canonical(position: &Position, pmn: &str) -> Result<Move, PmnError> {
    let mv = from_pmn(position, pmn)?;
    let canonical = to_pmn(position, &mv).map_err(PmnError::Illegal)?;
    if canonical == pmn {
        Ok(mv)
    } else {
        Err(PmnError::NotCanonical { canonical })
    }
}

/// Piece Identity without the transient state markers: letter (cased),
/// terminal status, derivation — what a move's actor may change (promotion),
/// as opposed to what the position's recompositions change.
fn same_identity(a: Epin, b: Epin) -> bool {
    cased_letter(a) == cased_letter(b)
        && a.is_terminal() == b.is_terminal()
        && a.is_derived() == b.is_derived()
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::{from_pmn, parse_canonical, to_pmn, well_formed, PmnError};
    use crate::domain::half_move::Move;
    use crate::position::Position;

    fn position(feen: &str) -> Position {
        Position::parse(feen).expect("valid FEEN")
    }

    fn content(s: &str) -> Move {
        Move::parse(s).expect("valid content")
    }

    /// (position, content, canonical PMN) — the examples of the rules document.
    const CASES: &[(&str, &str, &str)] = &[
        // E1 quiet
        (
            "4k^3/8/8/8/8/8/8/R3K^3 / W/w",
            r#"["a1","a4",null]"#,
            "a1-a4",
        ),
        // E1 capture, chess capturer: no suffix
        (
            "4k^3/8/8/8/r7/8/8/R3K^3 / W/w",
            r#"["a1","a4",null]"#,
            "a1+a4",
        ),
        // E1 capture, ōgi capturer of a chess piece: `/F`
        (
            "4k^3/8/8/8/r7/8/8/R3K^3 / J/w",
            r#"["a1","a4",null]"#,
            "a1+a4/F",
        ),
        // E1 capture, ōgi vs ōgi: flip
        (
            "4k^3/8/8/8/r7/8/8/R3K^3 / J/j",
            r#"["a1","a4",null]"#,
            "a1+a4/R",
        ),
        // E1 capture, chess capturing a tokin: demoted, victim's case
        (
            "4k^3/8/8/8/t7/8/8/R3K^3 / W/j",
            r#"["a1","a4",null]"#,
            "a1+a4/f",
        ),
        // E4 promotion, chess (choice)
        (
            "4k^3/P7/8/8/8/8/8/4K^3 / W/w",
            r#"["a7","a8","queen"]"#,
            "a7-a8=Q",
        ),
        // E4 promotion, ōgi (automatic, still written)
        (
            "4k^3/F7/8/8/8/8/8/4K^3 / J/w",
            r#"["a7","a8",null]"#,
            "a7-a8=T",
        ),
        // E4 promotion by capture, second player's fu taking a chess piece
        (
            "4k^3/8/8/8/8/8/1f6/RR2K^3 / j/W",
            r#"["b2","b1",null]"#,
            "b2+b1=t/f",
        ),
        // E2 castling
        (
            "4k^3/8/8/8/8/8/8/4K^2+R / W/w",
            r#"["e1","g1",null]"#,
            "e1~g1",
        ),
        // E3 en passant: the second player's pawn takes the marked pawn on b4
        (
            "4k^3/8/8/8/p-P6/8/8/4K^3 / w/W",
            r#"["a4","b3",null]"#,
            "a4~b3",
        ),
        // E5 drop, first then second player
        (
            "4k^3/8/8/8/8/8/8/4K^3 F/ J/w",
            r#"[null,"e4","fu"]"#,
            "F*e4",
        ),
        (
            "4k^3/8/8/8/8/8/8/4K^3 /f j/W",
            r#"[null,"e5","fu"]"#,
            "f*e5",
        ),
    ];

    #[test]
    fn to_pmn_matches_the_rules_document() {
        for (feen, mv, pmn) in CASES {
            let p = position(feen);
            assert_eq!(to_pmn(&p, &content(mv)).as_deref(), Ok(*pmn), "{feen} {mv}");
        }
    }

    #[test]
    fn from_pmn_reads_the_content_back() {
        for (feen, mv, pmn) in CASES {
            let p = position(feen);
            assert_eq!(from_pmn(&p, pmn), Ok(content(mv)), "{pmn}");
            assert_eq!(parse_canonical(&p, pmn), Ok(content(mv)), "{pmn}");
        }
    }

    #[test]
    fn non_canonical_spellings_are_refused() {
        let cases: &[(&str, &str, &str)] = &[
            ("4k^3/8/8/8/8/8/8/4K^2+R / W/w", "e1-g1", "e1~g1"),
            ("4k^3/8/8/8/r7/8/8/R3K^3 / W/w", "a1-a4", "a1+a4"),
            ("4k^3/8/8/8/r7/8/8/R3K^3 / J/w", "a1+a4", "a1+a4/F"),
            ("4k^3/8/8/8/r7/8/8/R3K^3 / J/w", "a1+a4/f", "a1+a4/F"),
            ("4k^3/F7/8/8/8/8/8/4K^3 / J/w", "a7-a8", "a7-a8=T"),
            ("4k^3/F7/8/8/8/8/8/4K^3 / J/w", "a7-a8=Q", "a7-a8=T"),
            ("4k^3/8/8/8/p-P6/8/8/4K^3 / w/W", "a4+b3", "a4~b3"),
            ("4k^3/8/8/8/p-P6/8/8/4K^3 / w/W", "a4-b3", "a4~b3"),
        ];
        for (feen, spelled, canonical) in cases {
            let p = position(feen);
            assert_eq!(
                parse_canonical(&p, spelled),
                Err(PmnError::NotCanonical {
                    canonical: (*canonical).to_owned()
                }),
                "{spelled}"
            );
        }
    }

    #[test]
    fn well_formed_is_the_position_free_half_of_from_pmn() {
        for ok in [
            "e2-e4",
            "e4+d5",
            "e1~g1",
            "a7-a8=Q",
            "b2+b1=t/f",
            "F*e4",
            "...",
            "+e2",
            "F.e4",
            "i9-i8",
            "+F*e4",
        ] {
            assert_eq!(well_formed(ok), Ok(()), "{ok}");
        }
        assert_eq!(well_formed("*e4"), Err(PmnError::DropWithoutPiece));
        for bad in [
            "e1e2", "e1-e1", "e1-", "e1-e2=Q/", "", "e2-e4=", "-e4", "Q*",
        ] {
            assert_eq!(well_formed(bad), Err(PmnError::Malformed), "{bad}");
        }
    }

    #[test]
    fn invalid_and_illegal_are_told_apart() {
        let p = position("4k^3/8/8/8/8/8/8/4K^3 F/ J/w");
        assert_eq!(from_pmn(&p, "*e4"), Err(PmnError::DropWithoutPiece));
        assert!(from_pmn(&p, "*e4").unwrap_err().is_invalid());
        assert_eq!(from_pmn(&p, "e1e2"), Err(PmnError::Malformed));
        assert_eq!(from_pmn(&p, "e1-e1"), Err(PmnError::Malformed));
        assert_eq!(from_pmn(&p, "e1-"), Err(PmnError::Malformed));
        assert_eq!(from_pmn(&p, "e1-e2=Q/"), Err(PmnError::Malformed));
        assert_eq!(from_pmn(&p, "i9-i8"), Err(PmnError::OffBoard));
        assert_eq!(from_pmn(&p, "..."), Err(PmnError::UnsupportedForm));
        assert_eq!(from_pmn(&p, "+e2"), Err(PmnError::UnsupportedForm));
        assert_eq!(from_pmn(&p, "F.e4"), Err(PmnError::UnsupportedForm));
        assert_eq!(from_pmn(&p, "+F*e4"), Err(PmnError::UnknownPiece));
        assert_eq!(from_pmn(&p, "F'*e4"), Err(PmnError::UnknownPiece));
        assert_eq!(from_pmn(&p, "Q*e4"), Err(PmnError::UnknownPiece));
        assert!(!from_pmn(&p, "Q*e4").unwrap_err().is_invalid());
        // Well-formed, wrong case for the dropper: reads as the piece, then
        // fails canonicity (the first player's hand holds `F`, written `F`).
        assert!(matches!(
            parse_canonical(&p, "f*e4"),
            Err(PmnError::NotCanonical { .. })
        ));
        // Well-formed, legal content, but an illegal move.
        assert!(matches!(
            parse_canonical(&p, "e1-e3"),
            Err(PmnError::Illegal(_))
        ));
    }
}
