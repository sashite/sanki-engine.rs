//! `Square` — a coordinate on the Sanki 8×8 board (`a1`–`h8`).
//!
//! *Pure* geometry: a file (`a`–`h`) and a rank (`1`–`8`), indexed `0..7`
//! internally (file 0 = `a`, rank 0 = `1`). This type knows nothing about FEEN
//! serialization or `Qi`'s flat index — the square ↔ index mapping lives in
//! [`crate::position::board`].
//!
//! The *square-string* format defined by `move-encoding-sanki.md` allows larger,
//! multidimensional boards; since Sanki is **8×8**, this type accepts only the
//! 2D subset `a1`–`h8` and rejects everything else (e.g. `i9`, `a1A`).

use core::fmt;

/// A square of the 8×8 board.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Square {
    file: u8, // 0..=7 (a..h)
    rank: u8, // 0..=7 (1..8)
}

/// Error returned when parsing a Sanki square-string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SquareError {
    /// The string is not exactly two bytes (a file followed by a rank).
    BadLength,
    /// The file character is not in `a`–`h`.
    BadFile,
    /// The rank character is not in `1`–`8`.
    BadRank,
}

impl Square {
    /// Number of files on the Sanki board.
    pub const FILE_COUNT: u8 = 8;
    /// Number of ranks on the Sanki board.
    pub const RANK_COUNT: u8 = 8;
    /// Total number of squares on the board (`FILE_COUNT` × `RANK_COUNT`).
    pub const SQUARE_COUNT: usize = 64;

    /// Builds a square from 0-based indices, or `None` if off the board.
    #[inline]
    #[must_use]
    pub const fn new(file: u8, rank: u8) -> Option<Self> {
        if file < Self::FILE_COUNT && rank < Self::RANK_COUNT {
            Some(Self { file, rank })
        } else {
            None
        }
    }

    /// File index, `0..=7` (`a` = 0).
    #[inline]
    #[must_use]
    pub const fn file(self) -> u8 {
        self.file
    }

    /// Rank index, `0..=7` (rank `1` = 0).
    #[inline]
    #[must_use]
    pub const fn rank(self) -> u8 {
        self.rank
    }

    /// Parses a Sanki square-string (`a1`–`h8`).
    ///
    /// # Errors
    /// Returns [`SquareError`] if the string is not exactly a file `a`–`h`
    /// followed by a rank `1`–`8`.
    pub fn parse(s: &str) -> Result<Self, SquareError> {
        match s.as_bytes() {
            [file, rank] => {
                let file = decode(*file, b'a', b'h').ok_or(SquareError::BadFile)?;
                let rank = decode(*rank, b'1', b'8').ok_or(SquareError::BadRank)?;
                // `decode` guarantees file, rank ∈ 0..=7, so `new` always succeeds.
                Self::new(file, rank).ok_or(SquareError::BadLength)
            }
            _ => Err(SquareError::BadLength),
        }
    }

    /// Offsets the square by `(df, dr)` (in indices), or `None` if it leaves the
    /// board.
    ///
    /// `df`/`dr` are signed: it is up to the caller to choose the orientation
    /// ("forward" depends on the side — see [`crate::position::style`]).
    #[inline]
    #[must_use]
    pub fn offset(self, df: i8, dr: i8) -> Option<Self> {
        let file = i16::from(self.file).checked_add(i16::from(df))?;
        let rank = i16::from(self.rank).checked_add(i16::from(dr))?;
        Self::from_signed(file, rank)
    }

    /// Enumerates the board's 64 squares (rank by rank, bottom to top, file
    /// `a`→`h`).
    pub fn all() -> impl Iterator<Item = Self> {
        (0..Self::RANK_COUNT)
            .flat_map(|rank| (0..Self::FILE_COUNT).map(move |file| Self { file, rank }))
    }

    /// Builds a square from signed coordinates, bounded to the board.
    fn from_signed(file: i16, rank: i16) -> Option<Self> {
        let file = u8::try_from(file).ok()?;
        let rank = u8::try_from(rank).ok()?;
        Self::new(file, rank)
    }
}

/// Decodes a `byte` expected in `lo..=hi` to its 0-based rank within that range.
fn decode(byte: u8, lo: u8, hi: u8) -> Option<u8> {
    if (lo..=hi).contains(&byte) {
        byte.checked_sub(lo)
    } else {
        None
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // self.file, self.rank ∈ 0..=7, so `wrapping_add` never overflows here.
        let file = char::from(b'a'.wrapping_add(self.file));
        let rank = char::from(b'1'.wrapping_add(self.rank));
        write!(f, "{file}{rank}")
    }
}

impl core::str::FromStr for Square {
    type Err = SquareError;
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for SquareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::BadLength => "a Sanki square must be exactly two characters (a1–h8)",
            Self::BadFile => "file outside a–h",
            Self::BadRank => "rank outside 1–8",
        };
        f.write_str(msg)
    }
}

impl core::error::Error for SquareError {}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::{Square, SquareError};

    #[test]
    fn parse_corners_and_center() {
        let cases = [
            ("a1", 0, 0),
            ("h1", 7, 0),
            ("a8", 0, 7),
            ("h8", 7, 7),
            ("e4", 4, 3),
            ("d5", 3, 4),
        ];
        for (s, file, rank) in cases {
            let sq = Square::parse(s).expect("valid square");
            assert_eq!(sq.file(), file, "{s} file");
            assert_eq!(sq.rank(), rank, "{s} rank");
        }
    }

    #[test]
    fn parse_rejects_invalid() {
        assert_eq!(Square::parse(""), Err(SquareError::BadLength));
        assert_eq!(Square::parse("a"), Err(SquareError::BadLength));
        assert_eq!(Square::parse("a1 "), Err(SquareError::BadLength));
        assert_eq!(Square::parse("a1A"), Err(SquareError::BadLength)); // 3D: outside Sanki
        assert_eq!(Square::parse("i1"), Err(SquareError::BadFile)); // larger board
        assert_eq!(Square::parse("A1"), Err(SquareError::BadFile)); // uppercase
        assert_eq!(Square::parse("a0"), Err(SquareError::BadRank));
        assert_eq!(Square::parse("a9"), Err(SquareError::BadRank));
        assert_eq!(Square::parse("aa"), Err(SquareError::BadRank));
        assert_eq!(Square::parse("11"), Err(SquareError::BadFile));
    }

    #[test]
    fn round_trips_over_the_whole_board() {
        for sq in Square::all() {
            let s = sq.to_string();
            assert_eq!(Square::parse(&s), Ok(sq), "round-trip {s}");
        }
        // The count of 64 is checked by `all_yields_64_distinct_squares`.
    }

    #[test]
    fn display_format() {
        assert_eq!(Square::new(0, 0).unwrap().to_string(), "a1");
        assert_eq!(Square::new(7, 7).unwrap().to_string(), "h8");
        assert_eq!(Square::new(4, 3).unwrap().to_string(), "e4");
    }

    #[test]
    fn new_bounds_the_board() {
        assert!(Square::new(7, 7).is_some());
        assert!(Square::new(8, 0).is_none());
        assert!(Square::new(0, 8).is_none());
    }

    #[test]
    fn offset_stays_on_board() {
        let e4 = Square::parse("e4").unwrap();
        assert_eq!(e4.offset(0, 1), Some(Square::parse("e5").unwrap()));
        assert_eq!(e4.offset(-1, -1), Some(Square::parse("d3").unwrap()));
        // Knight's jump.
        assert_eq!(e4.offset(1, 2), Some(Square::parse("f6").unwrap()));
        assert_eq!(e4.offset(-2, -1), Some(Square::parse("c3").unwrap()));
    }

    #[test]
    fn offset_off_board_is_none() {
        let a1 = Square::parse("a1").unwrap();
        assert_eq!(a1.offset(-1, 0), None);
        assert_eq!(a1.offset(0, -1), None);
        let h8 = Square::parse("h8").unwrap();
        assert_eq!(h8.offset(1, 0), None);
        assert_eq!(h8.offset(0, 1), None);
    }

    #[test]
    fn all_yields_64_distinct_squares() {
        let mut v: Vec<Square> = Square::all().collect();
        assert_eq!(v.len(), Square::SQUARE_COUNT);
        v.sort();
        v.dedup();
        assert_eq!(
            v.len(),
            Square::SQUARE_COUNT,
            "the 64 squares must be distinct"
        );
    }
}
