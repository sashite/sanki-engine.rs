#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![cfg_attr(not(test), warn(missing_docs))]

pub mod apply;
pub mod canonicalize;
pub mod capture;
pub mod clock;
pub mod domain;
pub mod engine;
pub mod error;
pub mod kernel;
pub mod legality;
pub mod movement;
pub mod position;
pub mod prelude;
pub mod terminal;
