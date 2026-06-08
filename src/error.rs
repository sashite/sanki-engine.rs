//! Boundary errors.
//!
//! The kernel and the layers below it never fail: an illegal move is reported in
//! the [`Outcome`](crate::domain::outcome)'s verdict, never as an `Err`. The only
//! failures are at the **boundary**, where the raw inputs handed to the arbiter
//! cannot be parsed into typed values before adjudication can even begin:
//!
//! - the initial position string is not a valid Sanki FEEN
//!   ([`FeenError`]);
//! - a ply's move content is malformed ([`MoveError`]);
//! - the session's time-control configuration is malformed
//!   ([`TimeControlError`]).
//!
//! Malformed-square failures are not a distinct case: they surface through
//! `MoveError` (a bad source/destination) or through FEEN decoding.

use crate::domain::half_move::MoveError;
use crate::domain::time_control::TimeControlError;
use crate::position::feen::FeenError;

/// A boundary failure: an input could not be parsed into a typed value before
/// adjudication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The position string was not a valid Sanki FEEN.
    Feen(FeenError),
    /// A ply's move content was malformed.
    Move(MoveError),
    /// The time-control configuration was malformed.
    TimeControl(TimeControlError),
}

impl From<FeenError> for Error {
    fn from(error: FeenError) -> Self {
        Self::Feen(error)
    }
}

impl From<MoveError> for Error {
    fn from(error: MoveError) -> Self {
        Self::Move(error)
    }
}

impl From<TimeControlError> for Error {
    fn from(error: TimeControlError) -> Self {
        Self::TimeControl(error)
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Feen(e) => write!(f, "invalid FEEN position: {e}"),
            Self::Move(e) => write!(f, "invalid ply content: {e}"),
            Self::TimeControl(e) => write!(f, "invalid time control: {e}"),
        }
    }
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Feen(e) => Some(e),
            Self::Move(e) => Some(e),
            Self::TimeControl(e) => Some(e),
        }
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

    use super::Error;
    use crate::domain::half_move::{Move, MoveError};
    use crate::domain::time_control::{TimeControl, TimeControlError};
    use crate::position::Position;
    use core::error::Error as _;

    /// A representative boundary step: parse a position then a ply, using `?` to
    /// funnel both parse errors through the single boundary [`Error`].
    fn boundary(feen: &str, content: &str) -> Result<(Position, Move), Error> {
        let position = Position::parse(feen)?;
        let mv = Move::parse(content)?;
        Ok((position, mv))
    }

    #[test]
    fn invalid_feen_surfaces_as_feen() {
        // A bad position string surfaces as Error::Feen via `?`.
        let result = boundary("garbage", "[\"e2\",\"e4\",null]");
        assert!(matches!(result, Err(Error::Feen(_))));
    }

    #[test]
    fn invalid_content_surfaces_as_move() {
        // A valid position but malformed move content surfaces as Error::Move.
        let result = boundary("4k^3/8/8/8/8/8/8/4K^3 / W/w", "not json");
        assert!(matches!(result, Err(Error::Move(_))));
    }

    #[test]
    fn valid_inputs_pass() {
        // Both inputs well-formed: no boundary error.
        let result = boundary("4k^3/8/8/8/8/8/8/4K^3 / W/w", "[\"e2\",\"e4\",null]");
        assert!(result.is_ok());
    }

    #[test]
    fn invalid_time_control_surfaces_as_time_control() {
        // An empty period list is a time-control boundary error.
        let error = Error::from(TimeControl::from_periods(Vec::new()).unwrap_err());
        assert_eq!(error, Error::TimeControl(TimeControlError::NoPeriods));
    }

    #[test]
    fn source_delegates_to_underlying_error() {
        // The boundary error chains to the underlying cause.
        let error = Error::Move(MoveError::NotJson);
        assert!(error.source().is_some());
    }

    #[test]
    fn display_is_non_empty() {
        let error = Error::Move(MoveError::TooLong);
        assert!(!error.to_string().is_empty());
    }
}
