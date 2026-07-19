//! `Status` — the Sanki termination vocabulary — and `Outcome3` — the three
//! possible outcomes.
//!
//! Mirrors `statuses-sanki.md`. The Adjudication's `content` field (kind `6425`)
//! is a lowercase ASCII string `^[a-z]{1,32}$`; each status has a unique
//! canonical form ([`Status::as_str`]). A Sanki session produces exactly one of
//! three outcomes — `100/0`, `50/50`, `0/100` ([`Outcome3`]); there is no partial
//! result.

use crate::domain::side::Side;

/// The termination cause of a Sanki session, as produced/recognized by the
/// Sashité arbiter in the Adjudication's `content`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    /// The loser's royal is in check and no pseudo-legal move escapes it.
    Checkmate,
    /// Pseudo-legal moves exist, the royal is not in check, but no legal move. Draw.
    Stalemate,
    /// No pseudo-legal move at all (the pseudo-legal set is empty). Draw.
    NoMove,
    /// Insufficient material to checkmate on either side. Draw.
    Insufficient,
    /// The same position reached the (threefold) repetition threshold. Draw.
    Repetition,
    /// 100 half-moves without a capture or an unpromoted foot-soldier move. Draw.
    MoveLimit,
    /// A player's time budget is exhausted. Decisive.
    Timeout,
    /// A player signalled they no longer intend to continue. Decisive.
    Resignation,
    /// Both players implicitly agreed to a draw. Draw.
    Agreement,
}

/// A status's origin, as categorized by `statuses-sanki.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusSource {
    /// Derived from evaluating the position under the variant's rules.
    RuleSystem,
    /// Derived from protocol mechanics (time, move legality).
    Protocol,
    /// Derived from protocol signals interpreted by the arbiter (resignation, agreement).
    ProtocolReserved,
}

/// The nature of the outcome associated with a status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResultKind {
    /// A draw outcome — always `50/50`.
    Draw,
    /// A decisive outcome — the loser is determined by context.
    Decisive,
}

impl Status {
    /// All statuses, in the order of the `statuses-sanki.md` table. Nine
    /// statuses exactly: `illegalmove` is deliberately NOT a status — an
    /// illegal ply never terminates a session (statuses-sanki §Verdict
    /// resolution: it is skipped by the selection rule, and at the kernel
    /// level it is a rejection that hands the state back).
    pub const ALL: [Self; 9] = [
        Self::Checkmate,
        Self::Stalemate,
        Self::NoMove,
        Self::Insufficient,
        Self::Repetition,
        Self::MoveLimit,
        Self::Timeout,
        Self::Resignation,
        Self::Agreement,
    ];

    /// The status's canonical form (the Adjudication's `content`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Checkmate => "checkmate",
            Self::Stalemate => "stalemate",
            Self::NoMove => "nomove",
            Self::Insufficient => "insufficient",
            Self::Repetition => "repetition",
            Self::MoveLimit => "movelimit",
            Self::Timeout => "timeout",
            Self::Resignation => "resignation",
            Self::Agreement => "agreement",
        }
    }

    /// Recognizes a Sanki status from its canonical form.
    ///
    /// # Errors
    /// Returns [`StatusError::Unknown`] if the string is not one of the nine
    /// Sanki vocabulary statuses (another protocol-valid `^[a-z]{1,32}$` string
    /// may belong to a different arbiter's vocabulary; the retired
    /// `"illegalmove"` token is likewise rejected).
    pub fn parse(s: &str) -> Result<Self, StatusError> {
        match s {
            "checkmate" => Ok(Self::Checkmate),
            "stalemate" => Ok(Self::Stalemate),
            "nomove" => Ok(Self::NoMove),
            "insufficient" => Ok(Self::Insufficient),
            "repetition" => Ok(Self::Repetition),
            "movelimit" => Ok(Self::MoveLimit),
            "timeout" => Ok(Self::Timeout),
            "resignation" => Ok(Self::Resignation),
            "agreement" => Ok(Self::Agreement),
            _ => Err(StatusError::Unknown),
        }
    }

    /// The status's origin (rule, protocol, protocol-reserved).
    #[must_use]
    pub const fn source(self) -> StatusSource {
        match self {
            Self::Checkmate
            | Self::Stalemate
            | Self::NoMove
            | Self::Insufficient
            | Self::Repetition
            | Self::MoveLimit => StatusSource::RuleSystem,
            Self::Timeout => StatusSource::Protocol,
            Self::Resignation | Self::Agreement => StatusSource::ProtocolReserved,
        }
    }

    /// The nature of the outcome: draw (`50/50`) or decisive.
    ///
    /// For a decisive outcome, the loser — hence the exact [`Outcome3`] — depends
    /// on context (see `statuses-sanki.md`, "Typical result" column).
    #[must_use]
    pub const fn result_kind(self) -> ResultKind {
        match self {
            Self::Checkmate | Self::Timeout | Self::Resignation => ResultKind::Decisive,
            Self::Stalemate
            | Self::NoMove
            | Self::Insufficient
            | Self::Repetition
            | Self::MoveLimit
            | Self::Agreement => ResultKind::Draw,
        }
    }
}

/// The three outcomes of a Sanki session — no partial outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Outcome3 {
    /// Decisive win for the first player (`100/0`).
    FirstWins,
    /// Draw (`50/50`).
    Draw,
    /// Decisive win for the second player (`0/100`).
    SecondWins,
}

impl Outcome3 {
    /// Point split `(first, second)`, summing to `100`.
    #[inline]
    #[must_use]
    pub const fn points(self) -> (u8, u8) {
        match self {
            Self::FirstWins => (100, 0),
            Self::Draw => (50, 50),
            Self::SecondWins => (0, 100),
        }
    }

    /// Decisive outcome in favor of `winner`.
    #[inline]
    #[must_use]
    pub const fn win_for(winner: Side) -> Self {
        match winner {
            Side::First => Self::FirstWins,
            Side::Second => Self::SecondWins,
        }
    }

    /// Decisive outcome at the expense of `loser`.
    #[inline]
    #[must_use]
    pub const fn loss_for(loser: Side) -> Self {
        match loser {
            Side::First => Self::SecondWins,
            Side::Second => Self::FirstWins,
        }
    }
}

/// Error returned when recognizing a Sanki status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusError {
    /// The string is not in the Sanki status vocabulary.
    Unknown,
}

impl core::fmt::Display for Status {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl core::str::FromStr for Status {
    type Err = StatusError;
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl core::fmt::Display for StatusError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("unknown Sanki termination status")
    }
}

impl core::error::Error for StatusError {}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::{Outcome3, ResultKind, Status, StatusError, StatusSource};
    use crate::domain::side::Side;

    #[test]
    fn canonical_forms_exact() {
        // Verbatim from statuses-sanki.md.
        assert_eq!(Status::Checkmate.as_str(), "checkmate");
        assert_eq!(Status::Stalemate.as_str(), "stalemate");
        assert_eq!(Status::NoMove.as_str(), "nomove");
        assert_eq!(Status::Insufficient.as_str(), "insufficient");
        assert_eq!(Status::Repetition.as_str(), "repetition");
        assert_eq!(Status::MoveLimit.as_str(), "movelimit");
        assert_eq!(Status::Timeout.as_str(), "timeout");
        assert_eq!(Status::Resignation.as_str(), "resignation");
        assert_eq!(Status::Agreement.as_str(), "agreement");
    }

    #[test]
    fn round_trip_str() {
        for &st in &Status::ALL {
            assert_eq!(Status::parse(st.as_str()), Ok(st), "{st}");
        }
    }

    #[test]
    fn content_satisfies_kind_6425_constraint() {
        // ^[a-z]{1,32}$
        for &st in &Status::ALL {
            let s = st.as_str();
            assert!((1..=32).contains(&s.len()), "{s} length");
            assert!(
                s.bytes().all(|b| b.is_ascii_lowercase()),
                "{s} ASCII lowercase",
            );
        }
    }

    #[test]
    fn parse_rejects_unknown() {
        assert_eq!(Status::parse(""), Err(StatusError::Unknown));
        assert_eq!(Status::parse("Checkmate"), Err(StatusError::Unknown));
        assert_eq!(Status::parse("checkmate "), Err(StatusError::Unknown));
        assert_eq!(Status::parse("win"), Err(StatusError::Unknown));
        assert_eq!(Status::parse("no_move"), Err(StatusError::Unknown));
        // The retired tenth status is no longer vocabulary.
        assert_eq!(Status::parse("illegalmove"), Err(StatusError::Unknown));
    }

    #[test]
    fn source_mapping() {
        use StatusSource::{Protocol, ProtocolReserved, RuleSystem};
        let expected = [
            (Status::Checkmate, RuleSystem),
            (Status::Stalemate, RuleSystem),
            (Status::NoMove, RuleSystem),
            (Status::Insufficient, RuleSystem),
            (Status::Repetition, RuleSystem),
            (Status::MoveLimit, RuleSystem),
            (Status::Timeout, Protocol),
            (Status::Resignation, ProtocolReserved),
            (Status::Agreement, ProtocolReserved),
        ];
        for (st, src) in expected {
            assert_eq!(st.source(), src, "{st}");
        }
    }

    #[test]
    fn result_kind_mapping() {
        use ResultKind::{Decisive, Draw};
        let expected = [
            (Status::Checkmate, Decisive),
            (Status::Stalemate, Draw),
            (Status::NoMove, Draw),
            (Status::Insufficient, Draw),
            (Status::Repetition, Draw),
            (Status::MoveLimit, Draw),
            (Status::Timeout, Decisive),
            (Status::Resignation, Decisive),
            (Status::Agreement, Draw),
        ];
        for (st, kind) in expected {
            assert_eq!(st.result_kind(), kind, "{st}");
        }
    }

    #[test]
    fn outcome3_points_and_constructors() {
        assert_eq!(Outcome3::FirstWins.points(), (100, 0));
        assert_eq!(Outcome3::Draw.points(), (50, 50));
        assert_eq!(Outcome3::SecondWins.points(), (0, 100));

        // The split always sums to 100.
        for o in [Outcome3::FirstWins, Outcome3::Draw, Outcome3::SecondWins] {
            let (a, b) = o.points();
            assert_eq!(u16::from(a).checked_add(u16::from(b)), Some(100), "{o:?}");
        }

        assert_eq!(Outcome3::win_for(Side::First), Outcome3::FirstWins);
        assert_eq!(Outcome3::win_for(Side::Second), Outcome3::SecondWins);
        assert_eq!(Outcome3::loss_for(Side::First), Outcome3::SecondWins);
        assert_eq!(Outcome3::loss_for(Side::Second), Outcome3::FirstWins);
    }
}
