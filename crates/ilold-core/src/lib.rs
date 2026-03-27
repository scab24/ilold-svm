//! Ilold Core — Smart contract execution path analysis engine.
//!
//! Pipeline: .sol files → Parser → Model Types → CFG Builder → CFG Graph

pub mod model;
pub mod parse;
pub mod cfg;
