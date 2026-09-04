//! Rule-system manifest generator — writes the `sanki` rule-system document
//! (`sashite.sanki.rules/1`, *Rule System — Sanki*) that a session names by
//! digest through its `rules` term (ADR-0033), together with the three GGN
//! documents it references.
//!
//! Usage:
//!
//! ```text
//! cargo run --release --example rules_manifest -- \
//!     --kernel <path to kernel-sanki.md> \
//!     --corpus <conformance directory> \
//!     [--out <output dir, default rules/>]
//! ```
//!
//! Output (deterministic — sorted keys, no insignificant whitespace, so that
//! regenerating reproduces every digest):
//!
//! - `<out>/ggn/{chess,ogi,xiongqi}.json` — the GGN documents;
//! - `<out>/corpus.json` — the conformance corpus as one object, category by
//!   category, whose SHA-256 is `kernel.conformance`;
//! - `<out>/sanki.rules.json` — the manifest; its SHA-256, printed last, is
//!   the value of the `rules` tag.
//!
//! The manifest carries **data, never code**: the parameters below are the
//! engine's own constants and tables, read from the crate, so that the
//! document and the implementation cannot drift apart silently.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

use sashite_sanki_engine::domain::variant::Variant;
use sashite_sanki_engine::ggn;
use sashite_sanki_engine::rules::{self, CORPUS_FILES};

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn compact(value: &Value) -> String {
    serde_json::to_string(value).expect("serialisable")
}

fn write(path: &Path, bytes: &[u8]) -> String {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("output directory");
    }
    std::fs::write(path, bytes).expect("write");
    let digest = sha256(bytes);
    println!("{}  {}  ({} bytes)", digest, path.display(), bytes.len());
    digest
}

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i.checked_add(1)?).cloned())
}

fn main() {
    let out = arg("--out").map_or_else(|| PathBuf::from("rules"), PathBuf::from);
    let kernel_path = arg("--kernel").expect("--kernel <path to kernel-sanki.md>");
    let corpus_dir = arg("--corpus").expect("--corpus <conformance directory>");

    // 1. The GGN documents, by digest.
    let mut ggn_digests = Vec::new();
    for variant in [Variant::Chess, Variant::Ogi, Variant::Xiongqi] {
        let json = compact(&ggn::to_json(&ggn::document(variant)));
        let name = rules::variant_name(variant);
        let digest = write(
            &out.join("ggn").join(format!("{name}.json")),
            json.as_bytes(),
        );
        ggn_digests.push((variant, digest));
    }

    // 2. The kernel specification and its conformance corpus, by digest.
    let kernel_bytes = std::fs::read(&kernel_path).expect("kernel specification");
    let kernel_digest = sha256(&kernel_bytes);
    println!("{kernel_digest}  {kernel_path}  (kernel specification)");
    let mut corpus = serde_json::Map::new();
    for file in CORPUS_FILES {
        let path = Path::new(&corpus_dir).join(file);
        let text = std::fs::read_to_string(&path).expect("corpus file");
        let value: Value = serde_json::from_str(&text).expect("corpus file is JSON");
        let category = file.trim_end_matches(".json").to_owned();
        corpus.insert(category, value);
    }
    let corpus_json = compact(&Value::Object(corpus));
    let conformance_digest = write(&out.join("corpus.json"), corpus_json.as_bytes());

    // 3. The manifest.
    let manifest = rules::manifest(&kernel_digest, &conformance_digest, &ggn_digests);
    let manifest_json = compact(&manifest);
    let rules_digest = write(&out.join("sanki.rules.json"), manifest_json.as_bytes());
    println!("rules digest: {rules_digest}");
}
