# Changelog

All notable changes to this crate are documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
crate adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **`ggn` — the GGN export of the three variants' movement tables** (ADR-0033,
  *Rule System — Sanki*). `ggn::document(variant)` generates the complete
  [Geometry Gameplay Notation 1.0.0](https://sashite.dev/specs/ggn/1.0.0/)
  document of a variant from the engine's own geometry — every piece, both
  sides, every state (`+`/`-` markers, the royal's check marker), every source
  including the hand, every destination, with the LCN pre-conditions of each
  possibility: blocking, capture, the double step keyed on `+`, *en passant*
  keyed on the victim's `-` (the three victim classes of cross-variant play),
  castling keyed on the rook's `+` and the royal's non-check state, ōgi drops.
  `ggn::satisfied`, `possible_destinations`, `possible_drops` and `attacked`
  evaluate a document against a position — `attacked` being the kernel's A1
  attack relation, derived from the document alone. Output is deterministic
  (sorted keys), about 1.2 MB per variant in plain JSON, ~80 KB gzipped.
- **`tests/ggn_differential.rs`** — the inventory's spike 10.6: on ~67 000
  positions reached by random play from the nine variant pairings, the moves
  the engine admits are exactly the GGN-possible moves the kernel's own filters
  (royal safety, uchifuzume, drop restrictions, hand availability) let through,
  and the engine's `is_attacked` equals the GGN-derived relation on every square
  for both sides. A GGN-possible move rejected for any other reason fails the
  test as an over-generation.

- **`rules` — the `sanki` rule-system manifest** (`sashite.sanki.rules/1`,
  *Rule System — Sanki*): `rules::manifest(kernel_digest, conformance_digest,
  ggn_digests)` builds the content-addressed document a session names through
  its `rules` term, from the engine's own constants and tables — the piece
  vocabulary (whose names are the actor vocabulary of Ply contents), the
  initial setups, the foot-soldier and promotion tables, the drop rules
  (`letters`, `nifu`, `uchifuzume`), the capture mutation, the dead-position
  predicates each pairing enables, and the session thresholds (`repetition`
  3, `move_limit` 100, `move_cap` 600, `candidate_cap` 8). Its tests pin the
  manifest to the engine: every named promotion target and droppable letter
  resolves through `ActorName`, every pairing's initial fragments assemble
  into a canonical, playable position, the thresholds are the engine's.
- **`examples/rules_manifest.rs`** — `cargo run --release --example
  rules_manifest -- --kernel <kernel-sanki.md> --corpus <conformance dir>
  [--out rules/]` writes the three GGN documents, the conformance corpus
  merged into one content-addressed `corpus.json`, and `sanki.rules.json`,
  printing every digest — the last one being the value of the `rules` tag.
  Deterministic: regenerating reproduces the digests. (`sha2` joins the
  dev-dependencies for it.)

### Noted

- **Nifu stays a kernel rule.** GGN matches a QPI value by exact token, and a
  same-side Fu may stand on a file as `F`, `+F` or `-F`; a `deny` object admits
  one value per square, so the three cannot be denied together. The document
  therefore carries the Fu drop's empty-square and last-rank conditions only;
  nifu is a kernel drop restriction the manifest parameterises (`drops.nifu`),
  like uchifuzume. The inventory's row 1.18 moves from "GGN" to "atom +
  parameter" accordingly.

## [0.10.2] — 2026-08-23

A test-data release, of the same nature as [0.10.1] (and as `v0.6.2`, which
re-embedded the corpus without ever reaching this file): **no code changes,
nothing behaves differently**. The shared conformance corpus gained
sixteen `xiongqi` legality vectors, and the client's cross-implementation gate
(`sanki.app.sveltekit`, `scripts/conformance-gate.sh`) compares its corpus
byte-for-byte against the one embedded in the **published** crate — so
re-embedding it *is* the release, and `conformance/kernels.json` is repointed
here.

### Added

- **`legality.json` v3 — sixteen `xiongqi` vectors, closing the coverage gap
  the corpus README named.** `xiongqi` had 8 legality vectors and all of them
  were about en passant or castling: nothing exercised the pieces the variant
  is actually made of. It now has 24 — the best-covered variant in the
  category, 51 vectors becoming 67.

  What they pin, and why each one earns its place:

  - **Bear** (4) — the diagonal slide, the refusal to jump, the absence of any
    orthogonal move (that is the Chariot's domain), and its freedom to **cross
    the river**. The last is the sharpest divergence from the xiàngqí Elephant
    the Bear replaces, which may never leave its own half; `rules-of-xiongqi.md`
    states that the river affects Soldiers *only*, and nothing tested it.
  - **Empress** (4) — the Chariot slide, an L-jump reaching `f2` from `d1` with
    **both** intervening squares occupied, the Chariot component blocked on its
    file, and **no diagonal slide at all**: the Empress is Chariot + Knight, not
    Chariot + Bear, which is its one real difference from the Western Queen.
  - **Soldier and the river** (4) — the sideways step illegal before the
    crossing and legal after it, the refusal to capture diagonally at any point
    of the board, and the double step.
  - **Promotion by choice** (4) — the *same* Soldier on the *same* square
    promoting to an Empress and to a Knight, both legal, which is what proves
    the choice is genuine rather than a fixed transformation (contrast the ōgi
    Fu, whose Tokin promotion is forced and carries no actor); plus `general`
    and `soldier` refused as targets.

- **`examples/gen_xiongqi_vectors.rs`** — the generator that produced them,
  shipped with them because the corpus's provenance rule requires it: inputs are
  curated with the designer's intent, expected values are computed by
  `kernel::step`, and **generation fails on any disagreement**. It did fail
  once, usefully — an Empress sliding `d1→d7` leaves a bare General covered on
  both a7 (rank) and b8 (knight jump), so the intended "ongoing" was in fact a
  stalemate; the vector gained a second black piece rather than the intent being
  asserted over the engine. Without this file in the crate, nobody can
  regenerate the sixteen.

### Changed

- **The corpus README's castling provenance note, discharged.** The nine
  `ogi-*`/`xiongqi-*` castling vectors and the `castling-canonicality-*` vector
  had carried, since v2, an exception saying they were spec-derived by hand
  because "the engine does not implement ōgi/xiongqi castling yet, so they are
  the target it must match". The engine has implemented castling for all three
  variants since 0.7.0 (`legality::castling`, `royal_letter()` mapping chess and
  ōgi to `K` and xiongqi to `G`), and the ten vectors agree with it. Confirmed
  by running the suite, and mutation-tested to prove the check has teeth:
  flipping `legality.ogi-kingside-castling-e1-g1` makes `legality_conformance`
  fail. They are hand-written and machine-confirmed; regenerating them is
  optional tidying, and a future disagreement is an ordinary bug again.

409 tests pass unchanged, the two normally-`#[ignore]`d heavy ones included
(`differential_rule_engine` — 9 cases, no divergence — and `perft_deep`).

## [0.10.1] — 2026-08-21

A documentation-only patch, promoted to a release by the conformance gate:
the client's cross-implementation gate (`sanki.app.sveltekit`,
`conformance/kernels.json`) pins crates.io releases whose embedded corpus is
byte-identical to the client's, and the kind renumbering below — inert for
behaviour — touched one conformance note (`clock.json`). Re-embedding the
corpus is the release; no code changes.

### Changed

- **Documentation only — the Event Timestamp Attestation kind is now `3410` (decision M-14, 2026-08-11).** The two doc-comment references (`clock`, `domain::time`) read `3410` instead of the NIP-03-adjacent `1041`. Same nature as the entry below: this crate holds no attestation constant, **nothing behaves differently**, and the correction ships with this patch.

- **Documentation only — the suite's kind numbers moved out of NIP-90's reserved range.** Every reference in the doc comments, the README and the conformance notes now reads `3418`–`3441` instead of `6418`–`6441`.

  This crate holds no kind constant — it is the rules engine, and it names a kind
  only to say which suite event a rule serves. **Nothing here behaves
  differently**, and a consumer on either numbering links against it unchanged.
  (Written to ride along with the next substantive release; the conformance
  gate promoted it to this patch instead — see the release note above.)

  [NIP-90](https://github.com/nostr-protocol/nips/blob/master/90.md) reserves
  `5000-7000` in one block and pairs a job request with its result at a fixed
  offset of a thousand, so a Ply at `6423` *was* the result of job request
  `5423` to anything that knows NIP-90 (`web-specs.md` README §Kind numbers).
  402 tests pass unchanged.

## [0.10.0] — 2026-08-07

Publishes the attack relation the crate already computed, in the two readings
its own callers could not reach.

### Added

- **`movement::attack::attackers_of(target, by, by_variant, piece_at)`** — the
  squares holding a piece of side `by` that attacks `target`, in board order.
  `is_attacked` answers whether that set is empty and nothing more, which is all
  check detection needs; a caller that must **count** the attackers or read
  their **kinds** had no way to ask. Both facts decide real questions about a
  position — a double check is two attackers, a smothered mate is one and it is
  a Knight — and answering them outside the crate meant reimplementing the
  dispatch, with a second table of letters to keep in step with this one.

- **`movement::attack::attacks_from(from, from_variant, target, piece_at)`** —
  the binary relation both other readings are built on: does the piece standing
  on `from` attack `target`. It was already here as the private `piece_attacks`,
  reachable only through a whole-board scan. Exposed directly, it also answers
  "does this piece **defend** that one", since the relation ignores the occupant
  of `target`.

  `from_variant` is the variant of the side the piece on `from` belongs to. The
  parameter earns its place on the foot-soldiers alone — a chess Pawn bears on
  its forward diagonals, an ōgi Fu straight ahead — so in a cross-variant
  position the other camp's variant answers about a different piece. The doc
  comment says so, and a test pins it.

### Changed

- Nothing. `is_attacked` keeps its signature, its short-circuit and its
  behaviour; `attackers_of` is a slower reading offered beside it, never a
  replacement. A test asserts the two never disagree — over every square of a
  mixed board, `is_attacked` equals the non-emptiness of `attackers_of` — and a
  second asserts `attacks_from` is exactly membership in `attackers_of`, over
  every ordered pair of squares.

## [0.9.0] — 2026-08-01

Moves the crate onto the reviewed notation stack — `sashite-feen` 0.2,
`sashite-qi` 0.2, and `sashite-sin` / `sashite-pin` / `sashite-epin` 1.1 — and
closes a latent defect that bump brought to the surface.

### Fixed

- **`Position::new` accepted boards that are not 8×8.** The geometry was checked
  only on the FEEN path, in `Position::parse`; the constructor itself validated
  the styles and nothing else. It is public, so this was reachable:

  ```rust
  let qi = Qi::new(&[9, 9], first, second)?;  // a shōgi board
  let position = Position::new(qi)?;          // accepted
  ```

  Everything downstream assumes 8×8 — `Square` addresses exactly 64 cells — so
  the resulting `Position` answered every lookup about squares that do not
  exist, and the whole rules engine ran on it. `Position::new` now rejects any
  other geometry with the new `PositionError::NotSankiBoard`, which makes
  "a `Position` is an 8×8 Sanki board" an invariant of the type rather than a
  property of one construction path. `Position::parse` keeps its own check as a
  cheap early-out — it rejects before `to_qi()` materializes a board that FEEN
  would allow to be 255×255 — and returns the same `FeenError::NotSankiBoard`
  as before, so nothing about its behaviour changes.

### Changed

- **`sashite-feen` 0.1 → 0.2 and `sashite-qi` 0.1 → 0.2**, necessarily together:
  FEEN 0.2 depends on and re-exports `Qi` 0.2, so bumping one alone would put
  two incompatible `Qi` types in the graph and `Feen::to_qi()` would stop
  matching this crate's own alias. `sashite-sin`, `sashite-pin` and
  `sashite-epin` move to 1.1 in the lockfile (their `"1"` requirement is
  unchanged).

  What the crate gains from them: an encoder that can no longer emit a FEEN
  string its own parser rejects, `Display` implementations that honour the
  format spec, and corrected error messages on the token types.

- **`sashite_feen::encode` is now fallible**, because `Qi` admits positions FEEN
  cannot spell. `Position::to_feen` keeps its `-> String` signature: with the
  geometry now guaranteed at construction, and 64 squares encoding to well under
  a kilobyte even when every one is occupied by a four-byte token, neither
  failure mode is reachable. The single `expect` this requires is the crate's
  first exception to its `expect_used = "deny"` policy; it is scoped with
  `#[expect(…, reason = …)]` — so it self-reports if it ever becomes redundant —
  and `to_feen_never_fails_on_any_reachable_position` asserts the implication it
  rests on (*whatever `Position::new` accepts, `encode` can spell*) rather than
  leaving it as prose. Removing either half of the geometry guard makes that
  test fail, which was verified by doing it.

### Added

- `new_rejects_any_board_that_is_not_8x8`, covering shōgi (9×9), xiangqi (10×9),
  one- and three-dimensional boards, the near misses 8×7 and 7×8, and a 64-cell
  board of the wrong shape — the right number of squares is not the invariant.

## [0.8.2] — 2026-07-31

Closes every remaining coverage gap flagged by the 0.8.1 reliability review.
Test-only: no production code changed, no doc changed, no API changed. Every
new fixture below was independently verified against the engine before being
committed as a hard assertion, and every one of them passes as specified —
this wave found zero further defects, only untested territory.

### Added

- **Cross-variant castling** (`legality::castling`), previously exercised
  only within a single variant at a time: `cross_variant_ogi_king_blocked_by_flying_general_on_transit_square`,
  `cross_variant_chess_king_blocked_by_ogi_fu_on_transit_square`,
  `cross_variant_xiongqi_general_blocked_by_chess_bishop_on_landing_square`,
  `cross_variant_chess_king_castles_despite_ogi_fu_attacking_off_path`.
- **Perft on a mixed pairing.** `tests/perft.rs` gains `MIXED_START`
  (the chess/ōgi FEEN already frozen in `golden_feen.rs`) and
  `perft_mixed_regression` (depths 1–2) in the default suite; the `#[ignore]`d
  `perft_deep` gains the same pairing at depth 3 (9,792) and depth 4
  (240,261). Ōgi's and xiongqi's own default-suite depths are unchanged —
  deepening either measured too slow for the non-ignored suite.
- **Uchifuzume against a chess King, against a xiongqi General, and from
  `Side::Second`** (all four combinations): `mating_fu_drop_is_uchifuzume_against_a_chess_king`,
  `..._against_a_xiongqi_general`, `..._for_a_second_side_dropper`,
  `..._for_a_second_side_dropper_against_a_xiongqi_general` in
  `terminal::uchifuzume`, plus the matching full-stack façade tests in
  `engine.rs`: `validate_rejects_a_mating_fu_drop_against_a_chess_king`,
  `..._against_a_xiongqi_general`, `validate_and_apply_reject_a_mating_fu_drop_by_a_second_side_dropper`,
  `legal_moves_exclude_the_mating_fu_drop_by_a_second_side_dropper`. The
  original uchifuzume tests only ever mated an ōgi King from `Side::First`.
- **`capture.rs`'s missing mirrored orderings**: `ogi_xiongqi_cj` and
  `chess_xiongqi_reversed` complete the direction matrix — every
  `VariantAssignment` pairing the capture-transform rule names is now tested
  in both orderings.
- **Movement edge cases**: file-a/file-h wraparound guards for the chess
  Pawn's diagonal capture and the post-river xiongqi Soldier's sideways
  capture (`movement::foot_soldier`); directional coverage for King, Queen,
  Bishop, Knight, Princess, and Tokin in `movement::generate` and
  `movement::attack`.
- **Three new conformance vectors** via `examples/gen_vectors.rs` —
  `legality.ogi-chess-capture-converts-to-a-droppable-ogi-fu`,
  `legality.ogi-xiongqi-general-captures-tokin-at-chariot-range`,
  `legality.mixed-uchifuzume-ogi-fu-drop-mates-xiongqi-general-is-illegal` —
  each verified against the engine by the generator itself before being
  written. `corpus-additions/legality-additions.json` now holds 25 vectors;
  merging them into the shared `tests/conformance/legality.json` corpus
  remains out of scope for this generator (a separate PR against the shared
  spec, as before).
- **Three new `tests/corpus/differential.tsv` seed lines**: a pure xiongqi
  quiet move, a mixed ōgi/xiongqi quiet move, and a mixed xiongqi/chess
  back-rank checkmate. The file's header and each mixed-pairing line now say
  explicitly that this widens self-consistent variety only — it is not a
  cross-implementation proof without a genuine second reference engine.

## [0.8.1] — 2026-07-31

A maintainer-requested reliability re-read of the whole crate. No behaviour
changes in a release build; everything below is documentation, an added
debug-only safety net, and new regression coverage.

### Fixed

- **`README.md`'s install snippet still pinned `sashite-sanki-engine = "0.7"`**
  after the 0.8.0 bump.
- **`Square::offset`'s doc comment pointed at the wrong module** for per-side
  "forward" orientation (`position::style`, which has no such concept) — it's
  `movement::forward`.
- **`Verdict::is_consistent`'s doc claimed an invariant its own constructors
  didn't enforce.** `Verdict::drawn`/`Verdict::decisive` now `debug_assert!`
  that the `Status` passed in actually matches the constructor's `ResultKind`,
  so a mismatched pairing (e.g. `decisive(Status::Stalemate, ..)`) panics in
  debug/test builds instead of silently producing a `Verdict` that fails its
  own `is_consistent()`. No shipping call site was ever affected — every one
  was traced during the review; this is a tripwire against a future one.

### Added

- **Three regression tests closing a coverage gap in the 0.8.0 fix.** The
  original fix and its tests exercised exactly one direction: chess capturing
  against a mated ōgi side. `engine::status_checkmate_with_inert_tray_reversed_direction`
  (ōgi mated, chess holding the inert tray), `engine::status_checkmate_with_inert_tray_xiongqi_capturer`
  (xiongqi as the inert-tray capturer — `capture`'s "chess or xiongqi
  capturer" rule names both, but only chess had a test), and
  `engine::status_stalemate_with_inert_cross_variant_tray` (the same pattern
  reached through `has_pseudo_legal_move`/stalemate rather than checkmate).
  Each was independently confirmed to reproduce `Ongoing` against the
  published, unpatched `sashite-sanki-engine` 0.7.0 before being added here.

## [0.8.0] — 2026-07-31

### Fixed

- **A checkmate with a cross-variant inert tray on the board could be misread
  as `Ongoing`.** `engine::status` and the kernel's `classify_terminal` each
  built their droppable-move probe from the union of both hands.
  `crate::capture`'s inert-tray rule keeps a captured piece's *original* case
  when the capturer is chess or xiongqi (documented, intentional: the token
  can then never satisfy `belongs_to` for the capturer) — but that same token
  *does* satisfy `belongs_to` for the side it was captured **from**. Unioned,
  it read as a phantom droppable reserve for whichever side was actually to
  move, letting a genuine checkmate escape detection whenever the mated
  side's own hand was empty and the opponent's inert tray happened to carry
  material cased as the mated side. Observed on a real chess-versus-ōgi game
  (`1.Ne5-c6+ Kd8-e8 2.Qb8xc8#`): the position after `2.Qb8xc8` has zero
  legal moves and the mover in check, yet `status` answered `Ongoing`.

### Changed — breaking

- **`terminal::legal_set::has_full_legal_move` takes two hand parameters** —
  `hand` (`side`'s own) and `opponent_hand` (the *other* side's own, read only
  by the uchifuzume sub-check) — replacing the single, unioned `hand` the
  fixed bug depended on. `has_legal_move` and `has_pseudo_legal_move` keep
  their existing single-hand signature; their contract is now stated
  explicitly in the module doc: `hand` must be `side`'s own pieces, never a
  union of both. **Breaking** for any direct caller of `has_full_legal_move`
  — the façade's four entry points (`legal_moves`, `validate`, `apply`,
  `status`) are unaffected in shape, only in (corrected) behaviour.

### Added

- Regression coverage at three layers:
  `terminal::legal_set::opponent_hand_never_counts_as_the_side_to_moves_own_reserve`
  (a token cased as `side` must not manufacture a drop when it only appears in
  `opponent_hand`), `engine::status_checkmate_with_inert_cross_variant_tray`
  and `kernel::step::checkmate_terminates_game_with_inert_cross_variant_tray`
  (the real game line above, replayed through the position-only façade and
  through the session kernel respectively).

## [0.7.0] — 2026-07-27

### Added

- **Castling in ōgi and xiongqi** (deciders' ruling, 2026-07-27 —
  `rules-of-ogi.md` / `rules-of-xiongqi.md` § *Castling*): all three variants
  now castle with the FIDE mechanics — the chess and ōgi **King**, and the
  xiongqi **General** (`G^`), each with a `+R`-bearing rook-class corner piece.
  `legality::castling::resolve_castling` takes the mover's variant (which royal
  letter castles); resolution, `legal_moves` enumeration, the terminal legal-set
  probe, and the `+R`/`-R`/`R` marker recomposition all cover the three
  variants. Ōgi specifics: a Rook dropped from hand is plain `R` and never
  castles (drops never confer the right); an opposing drop can transiently
  block a castling (`-R`). Xiongqi specifics: a two-file General displacement
  onto an **occupied** square resolves as the ordinary Chariot-style capture,
  never as castling (castling lands on an empty square); the flying-general
  line blocks conditions 4–6 through ordinary attack detection; a
  promotion-born Chariot never acquires the right.

### Changed

- **Canonical initial FEENs** (`backend-logic.md` § *Initial positions*): the
  ōgi and xiongqi back ranks now start `-R…-R` like chess (right retained,
  castling blocked by the occupied squares in between). Golden and perft
  fixtures updated; the frozen ōgi/xiongqi perft counts are unchanged (from the
  standard start, no castling is reachable within the tested depths — kingside
  needs f1/g1 vacated first, which takes two own plies before the castle).
- **Vendored conformance corpus** re-synced to `legality.json` **v2**
  (2026-07-27): nine stale xiongqi back ranks now carry the castling markers,
  nine input positions with the active royal in check now carry the canonical
  `-K^`/`-G^` marker, the mutual flying-general state in
  `insufficiency-xiongqi-last-capture` is repaired (black General moved to a8 —
  facing Generals across an open file is an illegal state), and nine castling
  vectors are added. `examples/gen_vectors.rs` regenerates all 22 curated
  additions byte-identically to the corpus (provenance restored); its emitter
  now omits `result`/`status` on illegal vectors, per the corpus format.

## [0.6.1] — 2026-07-26

### Fixed

- **The clock anchor never rewinds (time-accounting §Elapsed time).** An applied
  PREMOVE carries an anterior attestation — timed before its predecessor even
  landed — and `SessionState::advance` anchored the successor on it verbatim,
  billing the NEXT mover for time before the position was theirs to answer: a
  4-second reply read as 13 and flagged (`played-Ply timeout`) on the flagged
  player's own half-move. The anchor now advances monotonically —
  `max(last_attestation, attestation_at)`: the moment the position became
  answerable. The premove itself still charges zero (the existing anterior
  clamp). Pinned across implementations by the shared conformance vector
  `scenario.premove-anchor-never-rewinds` (arbiter corpus), which fails on
  0.6.0 and passes here.

## [0.6.0] — 2026-07-22

### Added

- **Absolute move cap — `movecap` (300 full moves / 600 half-moves).** A new
  global termination, applied identically to every variant and cross-variant
  pairing: a game still ongoing once **600 half-moves** have been played is an
  automatic draw (`50 / 50`), whatever the position — the non-resetting ceiling
  the FIDE 50-move rule (`movelimit`) does not provide. It sits **last** in the
  terminal ordering, so a decisive ending on the 600th half-move outranks it.
  New module `terminal/move_cap` (`HALF_MOVE_CAP = 600`, `cap_reached`), the
  status `Status::MoveCap` (`"movecap"`, rule-system source, draw), and
  `SessionState::move_cap_reached` — derived from the existing half-move counter,
  with no new per-ply state. See `web-specs.md/rules/rules/sanki-global-rules.md`
  and `statuses-sanki.md`.

### Changed

- **BREAKING — `Status` gained the `MoveCap` variant.** Exhaustive matches over
  `Status` in downstream code must add a `MoveCap` arm, and `Status::ALL` is now
  of length **10** (was 9). The suite's arbiter/player crates consume the
  `Status` API (`as_str` / `parse` / `result_kind`), not variant matches, and are
  unaffected.
- **BREAKING — `terminal::TerminalConditions` gained the `move_cap_reached`
  field.** Literal constructions of `TerminalConditions` must set it: the kernel
  populates it from `SessionState::move_cap_reached`, and the position-only
  `engine::status` reports it absent (like the other history-dependent facts).

## [0.5.1] — 2026-07-19

### Fixed

- **Legal-set en passant agrees with `resolve` on crafted markers.** The
  pseudo-legal loop of `terminal/legal_set.rs` judged a foot-soldier's step
  onto an empty square as a quiet move (royal safety tested with the would-be
  en-passant victim still on the board). For the post-river xiongqi Soldier the
  sideways step onto the skipped square is pseudo-legal, so on a CRAFTED `-`
  marker whose capture would be self-check the enumeration could count an
  escape `resolve` rejects — classifying a checkmate as ongoing. The loop now
  resolves `en_passant_capture` for a foot-soldier landing on an empty square
  and folds the victim into the safety probe, exactly as `resolve` does.
  Unreachable on canonical positions (the canonicality conditions only set `-`
  when the capture is self-safe); defense in depth against crafted inputs
  (deciders' confirmation, 2026-07-19).

## [0.5.0] — 2026-07-19

Correctness release following a global audit of the crate against the rule
specifications (rules-of-*.md, interactions-*.md, playing-principles.md,
statuses-sanki.md).

### Fixed

- **Xiongqi-mechanism en passant now actually captures.** `resolve` ran the
  en-passant resolution only when the destination was not pseudo-legally
  reachable — correct for the chess Pawn (whose diagonal step onto an empty
  square is never pseudo-legal) but wrong for the post-river xiongqi Soldier,
  whose sideways step onto the empty skipped square IS pseudo-legal: the
  "capture" applied as a quiet step, the double-stepped victim survived (its
  `-` marker merely cleared), and the self-check filter judged the wrong
  board — so a Soldier EP capture that was the only escape from check was
  rejected while `status` counted it, letting the façade disagree with
  itself. The resolution now runs for every foot-soldier arrival on an empty
  square, independently of reachability; pure-xiongqi and cross-variant
  (Soldier takes Pawn / takes Fu) captures work end to end, victim credited
  to the capturer's tray. Defense in depth: the Soldier's sideways EP now
  also requires the crossed river explicitly.
- **No phantom drops for chess/xiongqi holders.** The legal-move existence
  predicates counted drops for any holder with an own-case piece in hand,
  while `resolve` rejects every non-ōgi drop — on a crafted position,
  `status` could report Ongoing where `legal_moves` was empty. Both
  `legal_set` and `resolve_drop` now gate drops on the holder's variant being
  ōgi, so the classification and the move list always agree.
- **Promotion requires a forward move onto the last rank** (playing-principles
  §5): a xiongqi Soldier stepping sideways ALONG the last rank (crafted
  positions only) no longer promotes nor demands an actor.
  `resolve_promotion` takes the source square to decide forwardness.
- **A castling right cannot survive the King being off home.** On a crafted
  FEEN carrying `+R`/`-R` with the King away from `e1`/`e8`, recomposition now
  strips the right permanently instead of downgrading it to "transiently
  blocked" forever.
- **Forward timestamp overflow saturates instead of clamping to zero.** The
  premove clamp (`elapsed = max(0, t − anchor)`) no longer swallows the
  pathological case of an anchor near `i64::MIN`: the elapsed saturates and
  the mover flags, rather than an astronomically late ply passing free.

### Changed — breaking

- **The `illegalmove` status is retired** — the vocabulary now carries exactly
  the nine statuses of statuses-sanki.md, which abolishes `illegalmove` ("an
  illegal Ply is skipped, never a loss"): `Status::IllegalMove` is removed,
  `Status::ALL` has nine entries, and `Status::parse("illegalmove")` is
  rejected.
- **`kernel::step` returns a new `StepResult` enum.** An illegal ply is a
  **rejection**, never a termination: `StepResult::Illegal { state, reason }`
  hands the untouched `SessionState` back (the player keeps the turn, clocks
  unmoved); `StepResult::Advanced { outcome, next }` carries an applied ply.
  `Outcome` loses its now-meaningless `reason` field and describes applied
  plies only.
- **`resolve_promotion` gains the `from` square** (forwardness above).

### Changed

- **Checkmate outranks a same-ply `insufficient`** — the King-vs-General
  simultaneity (a mating capture that also reduces to the cross-variant
  K-vs-G dead pair) is ruled `checkmate`, per the deciders' ruling of
  2026-07-19: the "declared drawn outright" reading of K-vs-G spares ongoing
  positions a forced shuffle, it does not annul a delivered mate. The
  classification order was already correct; its false justifying comment is
  fixed and the precedence is pinned by a test.
- The kernel is wired through `terminal::move_limit` and
  `terminal::repetition` for the half-move-clock reset rule and both
  thresholds — previously duplicated logic, now a single source of truth.
- The differential corpus/status vocabulary uses `rejected` for an illegal
  move (formerly the retired `illegalmove` token).

## [0.4.0] — 2026-07-19

### Changed — breaking

- **The `engine` façade now applies the full rule system, uchifuzume
  included.** `engine::validate`, `engine::apply`, and `engine::legal_moves`
  previously stopped at the `legality` layer's `resolve`, so a mating Fu drop
  (uchifuzume, ōgi) was reported legal by the façade while the kernel — and
  therefore the arbiter — rejected it. The three entry points now share a
  single `resolve_full` composition (resolve + uchifuzume guard), aligning the
  façade exactly with the kernel's legality. Façade consumers (clients,
  players) no longer need their own uchifuzume guard.
- **New `IllegalReason::Uchifuzume` variant** (the taxonomy refinement the
  enum's documentation announced), returned by the façade and by the kernel —
  which previously reported the case as `IllegalReason::IllegalDrop`. An
  exhaustive `match` on `IllegalReason` must add the new arm.

### Changed

- **Checkmate/stalemate classification is now uchifuzume-exact.**
  `terminal::legal_set` gains `has_full_legal_move` — the legal-move existence
  predicate under the full rule system, excluding uchifuzume drops — used by
  `engine::status` and the kernel's terminal classification. The plain
  `has_legal_move` keeps its resolve-level reading: it is the base the full
  reading refines, and the reading `is_uchifuzume`'s inner mate test uses for
  the opponent's escapes (exact there — no escape from an adjacent Fu check
  can be a drop — and what guarantees the two functions never recurse). The
  two readings differ only in the vanishingly rare configuration where a
  player's sole legal move would be a mating Fu drop that blocks a distant
  check.
- **`is_uchifuzume` gains the single-square fast gate**: a drop can only be an
  uchifuzume when the opponent's royal stands exactly one square forward of
  the drop square (a drop blocks lines but never discovers one), so at most
  one square per position is ever probed — making the guard essentially free.

### Added

- `terminal::uchifuzume::is_uchifuzume_drop(position, piece, to)` — the
  `Position`-level convenience the façade and the kernel share.

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
