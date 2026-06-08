//! Convenience re-exports of the most commonly used items.
//!
//! Importing the prelude brings the core value types into scope, along with the
//! [`engine`] façade as a module:
//!
//! ```
//! use sashite_sanki_engine::prelude::*;
//!
//! let position = Position::parse("4k^3/8/8/8/8/8/8/4K^3 / W/w").expect("valid FEEN");
//! let moves: Vec<Move> = engine::legal_moves(&position);
//! assert!(!moves.is_empty());
//! ```
//!
//! Less common items — the session kernel, clocks, and the parsing error types —
//! remain available under their own module paths.

pub use crate::domain::half_move::Move;
pub use crate::domain::outcome::{IllegalReason, Verdict};
pub use crate::domain::side::Side;
pub use crate::domain::status::{Outcome3, Status};
pub use crate::engine;
pub use crate::position::Position;
