//! Verifies a rule-system manifest against this engine (`rules::verify`).
//!
//! Usage: `cargo run --example verify_rules -- <sanki.rules.json>`
//!
//! Prints the manifest's digest — the `rules` tag value — when this engine
//! implements exactly the rules it parameterises, and the first differing
//! member otherwise (exit status 1).

#![allow(clippy::expect_used, clippy::print_stdout, clippy::print_stderr)]

use sashite_sanki_engine::rules;

fn main() {
    let path = std::env::args().nth(1).expect("<sanki.rules.json>");
    let bytes = std::fs::read(&path).expect("manifest file");
    match rules::verify(&bytes) {
        Ok(verified) => {
            println!("{}  {path}", verified.digest());
            println!(
                "kernel {}  conformance {}",
                verified.kernel_digest(),
                verified.conformance_digest()
            );
        }
        Err(error) => {
            eprintln!("refused: {error}");
            std::process::exit(1);
        }
    }
}
