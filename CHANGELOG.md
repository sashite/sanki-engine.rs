# Changelog

All notable changes to this crate are documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
crate adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] — 2026-07-13

### Changed — breaking

- **Dead-position detection is now variant-specific** (rules update of
  2026-07-13, rules-of-*.md §Dead-Position Detection). The former unified
  insufficient-material rule ("only the two royals on the board and no
  droppable hand") is replaced by per-pairing detection:
  - **pure chess** — King vs King, King + Bishop vs King, King + Knight vs
    King, and Kings + Bishops only with all Bishops on same-coloured squares
    (pieces in hand ignored: the chess hand is inert);
  - **pure ōgi** — **no detection**: captures never remove material from play,
    so no materially dead configuration is reachable from the standard start
    (previously, a composed lone-Kings-empty-hands position would have been
    declared drawn);
  - **pure xiongqi** — General vs General only (inert hand ignored);
  - **mixed pairings** — unchanged: only royals on the board and no droppable
    hand, preserving the King-vs-General ruling of 2026-07-10
    (interactions-*-xiongqi.md §End of Game).
- **`terminal::insufficient` renamed to `terminal::dead_position`**, and
  `is_insufficient_material(piece_at, first_hand, second_hand)` becomes
  `is_dead_position(variants, piece_at, first_hand, second_hand)` — the
  detection needs the session's `VariantAssignment`. The termination status
  keeps the protocol token `insufficient` (statuses-sanki.md); nothing changes
  on the wire.

### Added

- Legality vectors and a chain scenario for the chess material-only dead
  positions in `examples/gen_vectors.rs` / `corpus-additions/` — positive
  (K+B vs K, K+N vs K, same-coloured Bishops) and negative (two Knights,
  opposite-coloured Bishops, pure-ōgi lone Kings) cases, pending their merge
  into the shared conformance corpus.
- **Property-based tests** (the crate's first use of `proptest`) on
  `terminal::dead_position`: hands never influence the pure-chess verdict,
  side-swap invariance, a Queen/Rook/Pawn is never dead, same-coloured
  Bishops are dead (and one opposite-coloured Bishop revives), Knight pairs
  and mixes are never dead, pure ōgi never declares, and the mixed-pairing
  verdict is exactly the droppable-hand criterion.
- Kernel-level integration tests pinning the new behaviour through
  `kernel::step`: a capture into K+B vs K terminates with `insufficient`;
  pure-ōgi lone Kings play on.

### Documentation

- The 50-move rule is stated as **automatic** (`move_limit` docs no longer say
  the draw "may be declared"), matching the updated rules — the engine's
  behaviour was already automatic.

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
