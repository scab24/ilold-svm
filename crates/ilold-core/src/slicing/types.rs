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

/// Where a sliced statement was lifted from. The slicer walks both the
/// function body and the bodies of every applied modifier; entries carry
/// this tag so renderers can distinguish "real" function code from
/// modifier code that wraps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "name")]
pub enum StatementOrigin {
    FunctionBody,
    /// `name` is the modifier identifier (e.g. `updateReward`).
    Modifier(String),
}

impl Default for StatementOrigin {
    fn default() -> Self {
        StatementOrigin::FunctionBody
    }
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
    /// Function-body statement vs modifier-body statement. Defaults to
    /// `FunctionBody` so older serialized payloads keep deserializing.
    #[serde(default)]
    pub origin: StatementOrigin,
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
