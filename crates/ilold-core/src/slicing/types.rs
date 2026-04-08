use serde::{Deserialize, Serialize};

use crate::model::common::SourceSpan;

/// Direction of a dataflow slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SliceDirection {
    /// Statements whose values feed the target variable (sources → var).
    Backward,
    /// Statements whose values derive from the target variable (var → sinks).
    Forward,
    /// Both directions, reported separately in the result.
    Both,
}

/// Unique identifier for a statement inside a flattened function body.
///
/// The slicing pipeline pre-walks the function body in program order and
/// assigns every statement — including nested ones inside If / For /
/// While / Block / TryCatch — a single global index. A `StatementPath`
/// wraps that index as `vec![global_index]`. We keep it as a `Vec<usize>`
/// (rather than a bare `usize`) so the serialized shape stays open for a
/// future hierarchical scheme without breaking on-disk session formats.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StatementPath(pub Vec<usize>);

impl StatementPath {
    pub fn new(indices: Vec<usize>) -> Self {
        StatementPath(indices)
    }
}

/// One statement in a slice result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceEntry {
    pub path: StatementPath,
    #[serde(default)]
    pub span: Option<SourceSpan>,
    /// Rendered source-like text of the statement (e.g. `reserve0 = uint112(balance0)`).
    pub text: String,
}

/// Full slice result for a (function, variable) query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceResult {
    pub function: String,
    pub variable: String,
    pub direction: SliceDirection,
    /// Backward slice entries in program order. Empty if direction is Forward.
    pub backward: Vec<SliceEntry>,
    /// Forward slice entries in program order. Empty if direction is Backward.
    pub forward: Vec<SliceEntry>,
}
