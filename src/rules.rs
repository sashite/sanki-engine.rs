//! The `sanki` rule-system document — `sashite.sanki.rules/1` (*Rule System —
//! Sanki*, ADR-0033): the content-addressed manifest a session names through
//! its `rules` term, built from the engine's own constants and tables so that
//! document and implementation cannot drift apart.
//!
//! The manifest is **data, never code**. It names the kernel by identifier and
//! by the digests of its specification and conformance corpus, references each
//! variant's GGN movement table by digest, and carries the parameters the
//! kernel reads: the piece vocabulary (whose names are the actor vocabulary of
//! Ply contents), the initial setups, the foot-soldier and promotion tables,
//! the drop rules, the capture mutation, the dead-position predicates enabled
//! per pairing, and the session-wide thresholds. `cargo run --release
//! --example rules_manifest` writes it with its digests.

use crate::domain::variant::Variant;
use crate::ggn;
use crate::terminal::move_cap::HALF_MOVE_CAP;
use crate::terminal::move_limit::HALF_MOVE_LIMIT;
use crate::terminal::repetition::THREEFOLD;
use serde_json::{json, Value};

/// The manifest format this module writes.
pub const FORMAT: &str = "sashite.sanki.rules/1";
/// The kernel the manifest parameterises — this engine.
pub const KERNEL_ID: &str = "sashite.sanki.kernel/1";
/// The slot selection cap `K` (*Move Encoding — Sanki* §Bounding a slot's
/// candidates), carried by the document so every consumer of a session uses
/// the same one.
pub const CANDIDATE_CAP: u32 = 8;
/// The conformance-corpus categories, in the order they are merged into the
/// content-addressed `corpus.json`.
pub const CORPUS_FILES: [&str; 6] = [
    "legality.json",
    "selection.json",
    "time.json",
    "scenarios.json",
    "clock.json",
    "puzzle.json",
];

/// The manifest name of a variant.
#[must_use]
pub const fn variant_name(variant: Variant) -> &'static str {
    match variant {
        Variant::Chess => "chess",
        Variant::Ogi => "ogi",
        Variant::Xiongqi => "xiongqi",
    }
}

/// One variant's definition (*Rule System — Sanki* §Variants).
#[must_use]
pub fn variant_entry(variant: Variant, ggn_digest: &str) -> Value {
    let style = ggn::style_letter(variant).to_string();
    match variant {
        Variant::Chess => json!({
            "style": style,
            "pieces": {
                "K": { "name": "king", "royal": true },
                "Q": { "name": "queen", "royal": false },
                "R": { "name": "rook", "royal": false },
                "B": { "name": "bishop", "royal": false },
                "N": { "name": "knight", "royal": false },
                "P": { "name": "pawn", "royal": false }
            },
            "ggn": ggn_digest,
            "initial": {
                "first": "+P+P+P+P+P+P+P+P/-RNBQK^BN-R",
                "second": "-rnbqk^bn-r/+p+p+p+p+p+p+p+p"
            },
            "foot_soldier": { "letter": "P", "home_rank": { "first": 2, "second": 7 } },
            "promotion": { "P": { "rank": { "first": 8, "second": 1 }, "targets": ["Q", "R", "B", "N"] } },
            "drops": { "letters": [], "nifu": [], "uchifuzume": [] },
            "capture": { "side": "keep", "demote": {}, "foreign": "keep" }
        }),
        Variant::Ogi => json!({
            "style": style,
            "pieces": {
                "K": { "name": "king", "royal": true },
                "I": { "name": "princess", "royal": false },
                "R": { "name": "rook", "royal": false },
                "B": { "name": "bishop", "royal": false },
                "N": { "name": "knight", "royal": false },
                "F": { "name": "fu", "royal": false },
                "T": { "name": "tokin", "royal": false }
            },
            "ggn": ggn_digest,
            "initial": {
                "first": "+F+F+F+F+F+F+F+F/-RNBIK^BN-R",
                "second": "-rnbik^bn-r/+f+f+f+f+f+f+f+f"
            },
            "foot_soldier": { "letter": "F", "home_rank": { "first": 2, "second": 7 } },
            "promotion": { "F": { "rank": { "first": 8, "second": 1 }, "targets": ["T"] } },
            "drops": { "letters": ["F", "R", "B", "N", "I"], "nifu": ["F"], "uchifuzume": ["F"] },
            "capture": { "side": "flip", "demote": { "T": "F" }, "foreign": "F" }
        }),
        Variant::Xiongqi => json!({
            "style": style,
            "pieces": {
                "G": { "name": "general", "royal": true },
                "E": { "name": "empress", "royal": false },
                "R": { "name": "chariot", "royal": false },
                "B": { "name": "bear", "royal": false },
                "N": { "name": "knight", "royal": false },
                "S": { "name": "soldier", "royal": false }
            },
            "ggn": ggn_digest,
            "initial": {
                "first": "+S+S+S+S+S+S+S+S/-RNBEG^BN-R",
                "second": "-rnbeg^bn-r/+s+s+s+s+s+s+s+s"
            },
            "foot_soldier": { "letter": "S", "home_rank": { "first": 2, "second": 7 } },
            "promotion": { "S": { "rank": { "first": 8, "second": 1 }, "targets": ["R", "N", "B", "E"] } },
            "drops": { "letters": [], "nifu": [], "uchifuzume": [] },
            "capture": { "side": "keep", "demote": {}, "foreign": "keep" }
        }),
    }
}

/// The dead-position predicates of the kernel's catalogue each pairing
/// enables (`terminal/dead_position.rs`).
#[must_use]
pub fn pairings() -> Value {
    json!({
        "chess+chess": { "dead_positions": ["lone_royals", "royal_and_bishop_vs_royal", "royal_and_knight_vs_royal", "royals_and_same_colour_bishops"] },
        "ogi+ogi": { "dead_positions": [] },
        "xiongqi+xiongqi": { "dead_positions": ["lone_royals"] },
        "chess+ogi": { "dead_positions": ["royals_only_with_inert_hands"] },
        "chess+xiongqi": { "dead_positions": ["royals_only_with_inert_hands"] },
        "ogi+xiongqi": { "dead_positions": ["royals_only_with_inert_hands"] }
    })
}

/// The complete manifest, given the digests it references: the kernel
/// specification's, the conformance corpus's, and each variant's GGN
/// document's (`[(variant, digest)]`).
#[must_use]
pub fn manifest(
    kernel_digest: &str,
    conformance_digest: &str,
    ggn_digests: &[(Variant, String)],
) -> Value {
    let mut variants = serde_json::Map::new();
    for (variant, digest) in ggn_digests {
        variants.insert(
            variant_name(*variant).to_owned(),
            variant_entry(*variant, digest),
        );
    }
    json!({
        "format": FORMAT,
        "game": "sanki",
        "name": "Sanki — Sashité reference rules",
        "kernel": {
            "id": KERNEL_ID,
            "digest": kernel_digest,
            "conformance": conformance_digest
        },
        "variants": Value::Object(variants),
        "pairings": pairings(),
        "session": {
            "stalemate": "draw",
            "repetition": THREEFOLD,
            "move_limit": HALF_MOVE_LIMIT,
            "move_cap": HALF_MOVE_CAP,
            "irreversible": { "capture": true, "foot_soldier_move": true, "drop": false },
            "candidate_cap": CANDIDATE_CAP
        }
    })
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::*;
    use crate::domain::actor::ActorName;
    use crate::position::Position;

    fn sample() -> Value {
        let g = |v: Variant| (v, format!("{:064x}", u8::from(v == Variant::Ogi)));
        manifest(
            "k",
            "c",
            &[g(Variant::Chess), g(Variant::Ogi), g(Variant::Xiongqi)],
        )
    }

    #[test]
    fn piece_names_are_the_actor_vocabulary_of_each_variant() {
        let m = sample();
        for variant in [Variant::Chess, Variant::Ogi, Variant::Xiongqi] {
            let pieces = m["variants"][variant_name(variant)]["pieces"]
                .as_object()
                .unwrap();
            // Every promotion target and droppable letter resolves through
            // the actor vocabulary to the same letter.
            let entry = &m["variants"][variant_name(variant)];
            let mut named: Vec<&str> = Vec::new();
            for (_, p) in entry["promotion"].as_object().unwrap() {
                let targets = p["targets"].as_array().unwrap();
                // A single target promotes automatically and is never named
                // by an actor (ōgi's Tokin); several are, and must resolve.
                if targets.len() > 1 {
                    named.extend(targets.iter().map(|t| t.as_str().unwrap()));
                }
            }
            named.extend(
                entry["drops"]["letters"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|t| t.as_str().unwrap()),
            );
            for letter in named {
                let name = pieces[letter]["name"].as_str().unwrap();
                let actor = ActorName::parse(name).unwrap();
                assert_eq!(
                    actor.letter_for(variant),
                    letter.chars().next(),
                    "{variant:?} {letter}"
                );
            }
        }
    }

    #[test]
    fn initial_fragments_assemble_into_valid_positions() {
        let m = sample();
        for first in [Variant::Chess, Variant::Ogi, Variant::Xiongqi] {
            for second in [Variant::Chess, Variant::Ogi, Variant::Xiongqi] {
                let f = m["variants"][variant_name(first)]["initial"]["first"]
                    .as_str()
                    .unwrap();
                let s = m["variants"][variant_name(second)]["initial"]["second"]
                    .as_str()
                    .unwrap();
                let feen = format!(
                    "{s}/8/8/8/8/{f} / {}/{}",
                    ggn::style_letter(first),
                    ggn::style_letter(second).to_ascii_lowercase()
                );
                let position = Position::parse(&feen).expect("valid initial position");
                assert_eq!(position.to_feen(), feen, "canonical as written");
                assert!(!crate::engine::legal_moves(&position).is_empty());
            }
        }
    }

    #[test]
    fn every_pairing_has_a_dead_position_entry() {
        let m = sample();
        let pairings = m["pairings"].as_object().unwrap();
        assert_eq!(pairings.len(), 6);
        for key in [
            "chess+chess",
            "ogi+ogi",
            "xiongqi+xiongqi",
            "chess+ogi",
            "chess+xiongqi",
            "ogi+xiongqi",
        ] {
            assert!(pairings[key]["dead_positions"].is_array(), "{key}");
        }
    }

    #[test]
    fn session_thresholds_are_the_engine_constants() {
        let m = sample();
        assert_eq!(m["session"]["repetition"], THREEFOLD);
        assert_eq!(m["session"]["move_limit"], HALF_MOVE_LIMIT);
        assert_eq!(m["session"]["move_cap"], HALF_MOVE_CAP);
        assert_eq!(m["session"]["candidate_cap"], CANDIDATE_CAP);
    }
}
