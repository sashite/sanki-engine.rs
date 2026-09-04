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
//!
//! The module is also the **consumer's** side of the document: [`verify`]
//! accepts a manifest iff it parameterises exactly this engine — same format,
//! same kernel, GGN tables whose digests are those of the tables this engine
//! generates, every other member equal to what [`document`] writes — so that a
//! session founded under a `rules` digest this engine does not implement is
//! refused rather than misjudged; and [`initial_position`] assembles the
//! position a Game Session must prescribe for a pairing from the variants'
//! initial fragments (*Rule System — Sanki* §Initial position).

use crate::domain::variant::Variant;
use crate::ggn;
use crate::position::feen::FeenError;
use crate::position::Position;
use crate::terminal::move_cap::HALF_MOVE_CAP;
use crate::terminal::move_limit::HALF_MOVE_LIMIT;
use crate::terminal::repetition::THREEFOLD;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

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

/// The initial fragments of a variant — the two ranks its army occupies at
/// the start, as the first player (ranks 2 and 1, in FEEN order) and as the
/// second (ranks 8 and 7).
#[must_use]
pub const fn initial_fragments(variant: Variant) -> (&'static str, &'static str) {
    match variant {
        Variant::Chess => (
            "+P+P+P+P+P+P+P+P/-RNBQK^BN-R",
            "-rnbqk^bn-r/+p+p+p+p+p+p+p+p",
        ),
        Variant::Ogi => (
            "+F+F+F+F+F+F+F+F/-RNBIK^BN-R",
            "-rnbik^bn-r/+f+f+f+f+f+f+f+f",
        ),
        Variant::Xiongqi => (
            "+S+S+S+S+S+S+S+S/-RNBEG^BN-R",
            "-rnbeg^bn-r/+s+s+s+s+s+s+s+s",
        ),
    }
}

/// The FEEN of the initial position of a pairing: the second player's fragment
/// on ranks 8–7, four empty ranks, the first player's fragment on ranks 2–1,
/// empty hands, the styles, `first` to move — the position a Game Session
/// (kind `3422`) MUST prescribe, and the one every verifier compares its
/// content to (*Rule System — Sanki* §Initial position).
#[must_use]
pub fn initial_feen(first: Variant, second: Variant) -> String {
    let (f, _) = initial_fragments(first);
    let (_, s) = initial_fragments(second);
    format!(
        "{s}/8/8/8/8/{f} / {}/{}",
        ggn::style_letter(first),
        ggn::style_letter(second).to_ascii_lowercase()
    )
}

/// The initial position of a pairing, decoded.
///
/// # Errors
///
/// The fragments being the engine's own constants, the decoding cannot fail;
/// the `Result` is the honest signature of [`Position::parse`], and the
/// `initial_fragments_assemble_into_valid_positions` test pins that every
/// pairing decodes and round-trips.
pub fn initial_position(first: Variant, second: Variant) -> Result<Position, FeenError> {
    Position::parse(&initial_feen(first, second))
}

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
    let initial = initial_fragments(variant);
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
            "initial": { "first": initial.0, "second": initial.1 },
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
            "initial": { "first": initial.0, "second": initial.1 },
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
            "initial": { "first": initial.0, "second": initial.1 },
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

/// The lowercase hex SHA-256 of `bytes` — the digest form of every
/// content-addressed document of the rule system (`rules` tag, `kernel.digest`,
/// `kernel.conformance`, `variants.*.ggn`).
#[must_use]
pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Whether `s` has the form of a digest: exactly 64 lowercase hex digits.
#[must_use]
pub fn is_digest(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The bytes of a document as published: compact JSON, keys sorted (the
/// `serde_json` map is ordered), no trailing newline — what the
/// `rules_manifest` example writes and what the digests are computed over.
#[must_use]
pub fn canonical_bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap_or_default()
}

/// The published bytes of `variant`'s GGN document.
#[must_use]
pub fn ggn_bytes(variant: Variant) -> Vec<u8> {
    canonical_bytes(&ggn::to_json(&ggn::document(variant)))
}

/// The digest of `variant`'s GGN document, as this engine generates it — the
/// value a manifest parameterising this engine carries in `variants.*.ggn`.
#[must_use]
pub fn ggn_digest(variant: Variant) -> String {
    sha256(&ggn_bytes(variant))
}

/// The complete manifest of this engine, computing the GGN digests itself:
/// [`manifest`] with `[(variant, ggn_digest(variant))]` for the three
/// variants. This is the document [`verify`] compares a candidate to.
#[must_use]
pub fn document(kernel_digest: &str, conformance_digest: &str) -> Value {
    let digests: Vec<(Variant, String)> = [Variant::Chess, Variant::Ogi, Variant::Xiongqi]
        .into_iter()
        .map(|variant| (variant, ggn_digest(variant)))
        .collect();
    manifest(kernel_digest, conformance_digest, &digests)
}

/// A manifest [`verify`] accepted: it parameterises exactly this engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    digest: String,
    kernel_digest: String,
    conformance_digest: String,
}

impl Verified {
    /// The digest of the manifest bytes — the value a session's `rules` tag
    /// carries when it plays under this document.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// `kernel.digest`: the kernel specification's digest, as the manifest
    /// states it (not checked here — the specification is a document this
    /// crate does not hold).
    #[must_use]
    pub fn kernel_digest(&self) -> &str {
        &self.kernel_digest
    }

    /// `kernel.conformance`: the conformance corpus's digest, as the manifest
    /// states it (not checked here, for the same reason).
    #[must_use]
    pub fn conformance_digest(&self) -> &str {
        &self.conformance_digest
    }
}

/// Why [`verify`] refused a manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulesError {
    /// The bytes are not a JSON object.
    Json,
    /// `format` is not [`FORMAT`].
    Format,
    /// `kernel.id` is not [`KERNEL_ID`]: the manifest parameterises a kernel
    /// this engine does not implement.
    Kernel,
    /// `kernel.digest` or `kernel.conformance` is absent or not a digest.
    Digest,
    /// The manifest is well-formed and names this kernel, but the named member
    /// differs from what this engine writes: it parameterises rules this
    /// engine does not implement (a different GGN table, a different
    /// threshold, another piece vocabulary…).
    Mismatch(String),
}

impl core::fmt::Display for RulesError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Json => write!(f, "the manifest is not a JSON object"),
            Self::Format => write!(f, "the manifest format is not {FORMAT}"),
            Self::Kernel => write!(f, "the manifest names a kernel other than {KERNEL_ID}"),
            Self::Digest => write!(f, "kernel.digest or kernel.conformance is not a digest"),
            Self::Mismatch(member) => {
                write!(
                    f,
                    "the manifest member {member} is not what this engine implements"
                )
            }
        }
    }
}

impl core::error::Error for RulesError {}

/// Accepts `bytes` as a rule-system manifest iff it parameterises **exactly
/// this engine**: it is a JSON object whose `format` is [`FORMAT`], whose
/// `kernel.id` is [`KERNEL_ID`], whose `kernel.digest` and
/// `kernel.conformance` have the form of digests, and which is, member for
/// member, the [`document`] this engine writes for those two digests — the
/// GGN digests included, which are those of the tables this engine generates.
///
/// A consumer holding a session's `rules` tag fetches the manifest by digest,
/// checks that [`Verified::digest`] equals the tag (content addressing), and
/// plays the session with this engine. Anything else — another format,
/// another kernel, a table or a threshold this engine does not implement — is
/// refused, so that no session is ever judged under rules other than its own.
///
/// # Errors
///
/// A [`RulesError`] naming the first check that failed; `Mismatch` carries
/// the path of the differing member (`session.move_cap`, `variants.ogi.ggn`…).
pub fn verify(bytes: &[u8]) -> Result<Verified, RulesError> {
    let candidate: Value = serde_json::from_slice(bytes).map_err(|_| RulesError::Json)?;
    let object = candidate.as_object().ok_or(RulesError::Json)?;
    if object.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(RulesError::Format);
    }
    let kernel = object
        .get("kernel")
        .and_then(Value::as_object)
        .ok_or(RulesError::Kernel)?;
    if kernel.get("id").and_then(Value::as_str) != Some(KERNEL_ID) {
        return Err(RulesError::Kernel);
    }
    let digest_member = |name: &str| -> Result<String, RulesError> {
        kernel
            .get(name)
            .and_then(Value::as_str)
            .filter(|s| is_digest(s))
            .map(str::to_owned)
            .ok_or(RulesError::Digest)
    };
    let kernel_digest = digest_member("digest")?;
    let conformance_digest = digest_member("conformance")?;
    let expected = document(&kernel_digest, &conformance_digest);
    if let Some(member) = first_difference(&expected, &candidate, "") {
        return Err(RulesError::Mismatch(member));
    }
    Ok(Verified {
        digest: sha256(bytes),
        kernel_digest,
        conformance_digest,
    })
}

/// The dotted path of the first member at which `actual` differs from
/// `expected`, or `None` when the two values are equal.
fn first_difference(expected: &Value, actual: &Value, path: &str) -> Option<String> {
    let name = |key: &str| {
        if path.is_empty() {
            key.to_owned()
        } else {
            format!("{path}.{key}")
        }
    };
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => {
            for (key, value) in e {
                match a.get(key) {
                    None => return Some(name(key)),
                    Some(other) => {
                        if let Some(inner) = first_difference(value, other, &name(key)) {
                            return Some(inner);
                        }
                    }
                }
            }
            a.keys()
                .find(|key| !e.contains_key(*key))
                .map(|key| name(key))
        }
        _ => (expected != actual).then(|| {
            if path.is_empty() {
                "(root)".to_owned()
            } else {
                path.to_owned()
            }
        }),
    }
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
                // The manifest's fragments and the engine's assembly agree.
                let f = m["variants"][variant_name(first)]["initial"]["first"]
                    .as_str()
                    .unwrap();
                let s = m["variants"][variant_name(second)]["initial"]["second"]
                    .as_str()
                    .unwrap();
                let feen = initial_feen(first, second);
                assert!(feen.starts_with(&format!("{s}/8/8/8/8/{f} / ")), "{feen}");
                let position = initial_position(first, second).expect("valid initial position");
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
    fn the_published_initial_position_is_the_well_known_one() {
        // The chess–ōgi pairing of the rules page, byte for byte.
        assert_eq!(
            initial_feen(Variant::Chess, Variant::Ogi),
            "-rnbik^bn-r/+f+f+f+f+f+f+f+f/8/8/8/8/+P+P+P+P+P+P+P+P/-RNBQK^BN-R / W/j"
        );
        assert_eq!(
            initial_feen(Variant::Xiongqi, Variant::Xiongqi),
            "-rnbeg^bn-r/+s+s+s+s+s+s+s+s/8/8/8/8/+S+S+S+S+S+S+S+S/-RNBEG^BN-R / C/c"
        );
    }

    const K: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const C: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

    #[test]
    fn verify_accepts_the_engines_own_document() {
        let bytes = canonical_bytes(&document(K, C));
        let verified = verify(&bytes).expect("the engine's own manifest verifies");
        assert_eq!(verified.digest(), sha256(&bytes));
        assert!(is_digest(verified.digest()));
        assert_eq!(verified.kernel_digest(), K);
        assert_eq!(verified.conformance_digest(), C);
        // Whitespace and key order are not the document: a pretty-printed,
        // reordered copy verifies too — under its own digest.
        let pretty = serde_json::to_vec_pretty(&document(K, C)).unwrap();
        let reprinted = verify(&pretty).expect("same members, same rules");
        assert_ne!(reprinted.digest(), verified.digest());
    }

    #[test]
    fn verify_refuses_anything_this_engine_does_not_implement() {
        let base = document(K, C);
        let mutate = |f: &dyn Fn(&mut Value)| {
            let mut m = base.clone();
            f(&mut m);
            verify(&canonical_bytes(&m))
        };
        assert_eq!(verify(b"not json"), Err(RulesError::Json));
        assert_eq!(verify(b"[]"), Err(RulesError::Json));
        assert_eq!(
            mutate(&|m| m["format"] = json!("sashite.sanki.rules/2")),
            Err(RulesError::Format)
        );
        assert_eq!(
            mutate(&|m| m["kernel"]["id"] = json!("sashite.sanki.kernel/2")),
            Err(RulesError::Kernel)
        );
        assert_eq!(
            mutate(&|m| m["kernel"]["digest"] = json!("ABC")),
            Err(RulesError::Digest)
        );
        assert_eq!(
            mutate(&|m| {
                m["kernel"].as_object_mut().unwrap().remove("conformance");
            }),
            Err(RulesError::Digest)
        );
        // A threshold this engine does not implement.
        assert_eq!(
            mutate(&|m| m["session"]["move_cap"] = json!(700)),
            Err(RulesError::Mismatch("session.move_cap".to_owned()))
        );
        // A GGN table that is not the one this engine generates.
        assert_eq!(
            mutate(&|m| m["variants"]["ogi"]["ggn"] = json!(K)),
            Err(RulesError::Mismatch("variants.ogi.ggn".to_owned()))
        );
        // A vocabulary difference, deep in the tree.
        assert_eq!(
            mutate(&|m| m["variants"]["chess"]["pieces"]["Q"]["name"] = json!("empress")),
            Err(RulesError::Mismatch(
                "variants.chess.pieces.Q.name".to_owned()
            ))
        );
        // A member added or removed.
        assert_eq!(
            mutate(&|m| m["session"]["extra"] = json!(1)),
            Err(RulesError::Mismatch("session.extra".to_owned()))
        );
        assert_eq!(
            mutate(&|m| {
                m["pairings"].as_object_mut().unwrap().remove("ogi+ogi");
            }),
            Err(RulesError::Mismatch("pairings.ogi+ogi".to_owned()))
        );
        // A variant removed.
        assert_eq!(
            mutate(&|m| {
                m["variants"].as_object_mut().unwrap().remove("xiongqi");
            }),
            Err(RulesError::Mismatch("variants.xiongqi".to_owned()))
        );
    }

    #[test]
    fn ggn_digests_are_stable_and_distinct() {
        let digests: Vec<String> = [Variant::Chess, Variant::Ogi, Variant::Xiongqi]
            .into_iter()
            .map(ggn_digest)
            .collect();
        assert!(digests.iter().all(|d| is_digest(d)));
        assert_ne!(digests[0], digests[1]);
        assert_ne!(digests[1], digests[2]);
        assert_eq!(ggn_digest(Variant::Ogi), digests[1], "deterministic");
    }

    #[test]
    fn is_digest_is_strict() {
        assert!(is_digest(K));
        assert!(!is_digest(&K.to_uppercase()));
        assert!(!is_digest(&K[..63]));
        assert!(!is_digest(&format!("{}g", &K[..63])));
        assert!(!is_digest(""));
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
