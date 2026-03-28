use serde::{Deserialize, Serialize};

use crate::model::function::Visibility;

/// All possible function call sequences for a contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceTree {
    pub contract: String,
    pub functions: Vec<SequenceFunction>,
    pub sequences: Vec<TransactionSequence>,
    pub max_depth: usize,
}

/// A callable function in the sequence tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceFunction {
    pub name: String,
    pub visibility: Visibility,
    pub read_only: bool,
    pub path_count: usize,
}

/// One specific ordering of function calls (e.g., deposit → withdraw → claim).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSequence {
    /// Indices into SequenceTree.functions
    pub steps: Vec<usize>,
    pub depth: usize,
    /// Product of each step's path_count
    pub path_count: u64,
    /// false only if ALL steps are read_only
    pub has_state_change: bool,
}
