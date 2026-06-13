# Sashité Sanki Engine

[![Crates.io](https://img.shields.io/crates/v/sashite-sanki-engine.svg)](https://crates.io/crates/sashite-sanki-engine)
[![Docs.rs](https://docs.rs/sashite-sanki-engine/badge.svg)](https://docs.rs/sashite-sanki-engine)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://github.com/sashite/sanki-engine.rs/blob/main/LICENSE)

Rules engine for the **Sanki** game suite — `chess`, `ogi`, and `xiongqi`, all
played on an 8×8 board — built for [Sashité](https://sashite.com/). A pure rules
engine with **no Nostr dependency**, published under the Apache-2.0 license.

The adjudication layer (race resolution, verdict assembly over an abstract event
model) lives in the companion crate
[`sashite-sanki-arbiter`](https://github.com/sashite/sanki-arbiter.rs), which
depends on this one.

## Architecture

The crate is layered, each layer building only on those below it:

- **L1 — kernel** (`kernel`): a pure per-ply transition
  (`legality → apply → canonicalize → tick → terminal`). No I/O, no Nostr.
- The `engine` module is the ergonomic **façade** over the kernel.

## Design guarantees

- **Panic-free by construction.** Crate lints forbid `unsafe`, and deny
  `unwrap`/`expect`/`panic`, slice indexing, and overflowing arithmetic. The
  kernel never fails on a well-formed input: an illegal move is reported in the
  outcome's *verdict*, never as an `Err`.
- **Deterministic.** Every entry point is a pure function of its inputs; the
  per-session concerns (clocks, the history that repetition and the move-limit
  depend on) live in the `kernel`.
- **Three variants, one engine.** Chess, ōgi, and xiongqi share a single rule
  set with no per-matchup branching; cross-variant interactions (captures, hand
  conversions) follow one common model.

## Usage

```toml
[dependencies]
sashite-sanki-engine = "0.2"
```

```rust
use sashite_sanki_engine::domain::half_move::Move;
use sashite_sanki_engine::domain::outcome::Verdict;
use sashite_sanki_engine::engine;
use sashite_sanki_engine::position::Position;

// A position is parsed from its canonical FEEN.
let position = Position::parse("4k^3/8/8/8/8/8/8/R3K^3 / W/w").expect("valid Sanki FEEN");

// Its intrinsic status: no checkmate, stalemate, or insufficient material here.
assert_eq!(engine::status(&position), Verdict::Ongoing);

// Every legal move for the side to move can be enumerated.
assert!(!engine::legal_moves(&position).is_empty());

// A move is a kind-6423 content array: [from, to, actor].
let mv = Move::parse(r#"["a1","a4",null]"#).expect("valid ply content");
assert!(engine::validate(&position, &mv).is_ok());

// Applying a legal move returns the canonical resulting position.
let next = engine::apply(&position, &mv).expect("legal move");
assert_eq!(next.to_feen(), "4k^3/8/8/8/R7/8/8/4K^3 / w/W");

// `status` also detects terminations — here a back-rank checkmate.
let mated = Position::parse("R6-k^/6pp/8/8/8/8/8/4K^3 / w/W").expect("valid FEEN");
assert!(engine::status(&mated).is_terminated());
```

The four entry points of `engine` are `legal_moves`, `validate`, `apply`, and
`status`. They are pure functions over a `Position`; for clocks, repetition, and
the move-limit, drive the `kernel` directly.

The core types above can be brought into scope at once with
`use sashite_sanki_engine::prelude::*;`, which also re-exports the `engine`
module.

## Input formats

- A **position** is a FEEN string (board, hands, and styles + active player),
  parsed by `Position::parse`.
- A **move** is the kind-6423 `content`: a three-element array
  `[from, to, actor]`, e.g. `["a1","a4",null]` for a board move or
  `[null,"h7","fu"]` for an ōgi drop. See the
  [Sashité specifications](https://sashite.com/) for the encodings.

## Adjudication

This crate validates and applies moves and reports a position's intrinsic status.
Session-level adjudication — ruling on a game from its public events (plies,
attestations, adjudication requests), ranking termination causes by attestation
time — is provided by the companion crate
[`sashite-sanki-arbiter`](https://github.com/sashite/sanki-arbiter.rs).

## Built on

`sashite-feen`, `sashite-qi`, `sashite-epin`, `sashite-sin`, with `serde`.

## Minimum supported Rust version

Rust 1.81.

## License

Licensed under the [Apache License, Version 2.0](https://github.com/sashite/sanki-engine.rs/blob/main/LICENSE).
