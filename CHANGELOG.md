# Changelog

All notable changes to this crate are documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
crate adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] — 2026-06-13

### Changed — breaking

- **`SessionState::step` renamed to `half_move`.** The accessor reports the
  1-based **play-order position** of the next ply (the count of half-moves so
  far, plus one) — it always did, but the name clashed with the revised
  Sanki vocabulary, where a Ply's kind-`6423` `step` is each signer's own move
  ordinal. The counter is unchanged; only `SessionState::step()` becomes
  `SessionState::half_move()`. The mapping from a play-order position to a
  `(signer, step)` slot is the consuming application's concern (e.g.
  `sashite-sanki-arbiter`).

## [0.1.1] — 2026-06-08

- Added a `prelude` of common re-exports, a tested README doctest, and the
  `deny.toml` cargo-deny configuration.

## [0.1.0] — 2026-06-08

Initial release: position model and FEEN, the `[source, destination, actor]`
move model, per-variant legal-move generation, move application and
canonicalization, terminal/status detection, time-control accounting, and the
pure per-ply kernel.
