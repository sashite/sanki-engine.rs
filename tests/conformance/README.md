# Vendored conformance corpus (legality)

`legality.json` is a **vendored copy** of the shared Sanki conformance vectors.
**Source of truth:** `web-specs.md/nostr/conformance/` — re-sync on any change.
Run by `tests/conformance.rs` against the engine's `validate` / `apply` / `status`.
The `selection.json` / `time.json` vectors are exercised by the arbiter crate.
