use petgraph::stable_graph::StableDiGraph;
use serde::{Deserialize, Serialize};

use crate::model::common::SourceSpan;
use crate::model::expression::AssignOperator;

pub type CfgGraph = StableDiGraph<BasicBlock, BranchEdge>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicBlock {
    pub id: usize,
    pub kind: BlockKind,
    pub statements: Vec<CfgStatement>,
    pub span: Option<SourceSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockKind {
    Entry,
    Normal,
    LoopCondition,
    Return,
    Revert,
    Assembly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BranchEdge {
    Unconditional,
    ConditionalTrue { condition: String },
    ConditionalFalse { condition: String },
    ExternalCallSuccess,
    ExternalCallFailure,
    LoopBack,
    LoopExit,
    CatchClause { kind: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CfgStatement {
    Assignment {
        target: String,
        value: String,
        operator: AssignOperator,
        span: Option<SourceSpan>,
        #[serde(default)]
        from_modifier: Option<String>,
    },
    ExternalCall {
        target: String,
        function: String,
        span: Option<SourceSpan>,
        #[serde(default)]
        from_modifier: Option<String>,
    },
    InternalCall {
        function: String,
        span: Option<SourceSpan>,
        #[serde(default)]
        from_modifier: Option<String>,
    },
    EmitEvent {
        event: String,
        span: Option<SourceSpan>,
        #[serde(default)]
        from_modifier: Option<String>,
    },
    StateRead {
        variable: String,
        span: Option<SourceSpan>,
        #[serde(default)]
        from_modifier: Option<String>,
    },
    StateWrite {
        variable: String,
        span: Option<SourceSpan>,
        #[serde(default)]
        from_modifier: Option<String>,
    },
    EthTransfer {
        to: String,
        span: Option<SourceSpan>,
        #[serde(default)]
        from_modifier: Option<String>,
    },
    RequireCheck {
        condition: String,
        message: Option<String>,
        span: Option<SourceSpan>,
        #[serde(default)]
        from_modifier: Option<String>,
    },
    AssertCheck {
        condition: String,
        span: Option<SourceSpan>,
        #[serde(default)]
        from_modifier: Option<String>,
    },
    AssemblyBlock {
        span: Option<SourceSpan>,
        #[serde(default)]
        from_modifier: Option<String>,
    },
}
