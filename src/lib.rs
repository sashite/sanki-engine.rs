//! `sashite-sanki-engine` — rules engine for the Sanki game suite, built for Sashité.
//!
//! Layers: `domain` -> `position` -> `movement` -> `legality`
//! -> `apply`/`canonicalize` -> `terminal` -> `kernel` (L1).
//!
//! The L2 adjudication layer lives in the companion `sashite-sanki-arbiter` crate,
//! which depends on this one.

#![forbid(unsafe_code)]
#![cfg_attr(not(test), warn(missing_docs))]

pub mod apply;
pub mod canonicalize;
pub mod capture;
pub mod clock;
pub mod domain;
pub mod engine;
pub mod error;
pub mod kernel;
pub mod legality;
pub mod movement;
pub mod position;
pub mod terminal;
