use petgraph::stable_graph::StableDiGraph;
use serde::{Deserialize, Serialize};

use crate::model::function::{Mutability, Visibility};

pub type CallGraph = StableDiGraph<CallNode, CallEdge>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallNode {
    pub contract: String,
    pub function: String,
    pub visibility: Visibility,
    pub mutability: Mutability,
    pub is_external: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallEdge {
    pub kind: CallKind,
    pub call_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallKind {
    Internal,
    External,
    Inherited,
}
