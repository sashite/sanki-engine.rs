//! GGN export — writes the Geometry Gameplay Notation (GGN 1.0.0) document of
//! each Sanki variant, the movement tables the `sanki` rule-system manifest
//! names by digest (*Rule System — Sanki* §GGN documents, ADR-0033).
//!
//! Usage: `cargo run --release --example ggn_export [-- <output-dir>]`
//! (default `ggn/`). Output is deterministic JSON (sorted keys, no
//! insignificant whitespace): regenerating a document yields the same bytes,
//! hence the same SHA-256. The documents are verified against the engine by
//! `tests/ggn_differential.rs`.

#![allow(clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use sashite_sanki_engine::domain::variant::Variant;
use sashite_sanki_engine::ggn;

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("ggn"), PathBuf::from);
    std::fs::create_dir_all(&dir).expect("output directory");
    for (variant, name) in [
        (Variant::Chess, "chess"),
        (Variant::Ogi, "ogi"),
        (Variant::Xiongqi, "xiongqi"),
    ] {
        let doc = ggn::document(variant);
        let json = serde_json::to_string(&ggn::to_json(&doc)).expect("serialisable");
        let path = dir.join(format!("{name}.json"));
        std::fs::write(&path, json.as_bytes()).expect("write");
        let possibilities: usize = doc
            .values()
            .flat_map(|s| s.values())
            .flat_map(|d| d.values())
            .map(Vec::len)
            .sum();
        println!(
            "{}: {} QPI keys, {possibilities} possibilities, {} bytes",
            path.display(),
            doc.len(),
            json.len()
        );
    }
}
