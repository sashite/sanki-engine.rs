//! L1 — pure per-ply kernel: `step(state, ply, attestation_at) -> Outcome`.
//! legality -> apply -> canonicalize -> tick clock -> terminal.
//! ALWAYS returns an `Outcome` (illegality encoded, never an Err).

pub mod state;
pub mod step;
