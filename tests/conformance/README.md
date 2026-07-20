# Vendored conformance corpus (legality, clock)

`legality.json` and `clock.json` are **vendored copies** of the shared Sanki
conformance vectors. **Source of truth:** `web-specs.md/nostr/conformance/` —
re-sync on any change. `legality.json` is run by `tests/conformance.rs` against
the engine's `validate` / `apply` / `status`; `clock.json` (category E, the
per-ply time-accounting arithmetic) by `tests/conformance_clock.rs` against
`clock::tick`. The `selection.json` / `time.json` / `scenarios.json` vectors are
exercised by the arbiter crate.
