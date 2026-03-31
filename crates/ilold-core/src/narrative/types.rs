use serde::{Deserialize, Serialize};
use std::fmt;

use crate::classify::entry_points::AccessLevel;
use crate::pathtree::types::TerminalKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionNarrative {
    pub contract: String,
    pub name: String,
    pub access: AccessLevel,
    pub total_paths: usize,
    pub happy_paths: usize,
    pub revert_paths: usize,
    pub paths: Vec<PathNarrative>,
    pub observations: Vec<Observation>,
    pub state_writes: Vec<String>,
    pub state_reads: Vec<String>,
    pub external_calls: Vec<String>,
    pub modifiers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathNarrative {
    pub id: usize,
    pub terminal: TerminalKind,
    pub steps: Vec<NarrativeStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeStep {
    pub step_type: StepType,
    pub description: String,
    pub detail: Option<String>,
    pub branch: Option<BranchDirection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepType {
    Entry,
    Condition,
    StateWrite,
    StateRead,
    ExternalCall,
    InternalCall,
    EthTransfer,
    Event,
    Return,
    Revert,
    Assembly,
}

impl StepType {
    pub fn icon(&self) -> &str {
        match self {
            StepType::Entry => "▶",
            StepType::Condition => "▸",
            StepType::StateWrite => "✏",
            StepType::StateRead => "◇",
            StepType::ExternalCall => "→",
            StepType::InternalCall => "○",
            StepType::EthTransfer => "Ξ",
            StepType::Event => "◆",
            StepType::Return => "✓",
            StepType::Revert => "✗",
            StepType::Assembly => "⚙",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchDirection {
    True,
    False,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub kind: ObservationKind,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationKind {
    CeiViolation,
    SharedState,
    NoAccessControl,
    ExternalCallRisk,
}

impl fmt::Display for ObservationKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ObservationKind::CeiViolation => write!(f, "CEI violation"),
            ObservationKind::SharedState => write!(f, "Shared state"),
            ObservationKind::NoAccessControl => write!(f, "No access control"),
            ObservationKind::ExternalCallRisk => write!(f, "External call"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceNarrative {
    pub contract: String,
    pub steps: Vec<SequenceStep>,
    pub observations: Vec<Observation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceStep {
    pub function: String,
    pub access: AccessLevel,
    pub requires: Vec<String>,
    pub effects: Vec<String>,
    pub external_calls: Vec<String>,
    pub events: Vec<String>,
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub from_step: usize,
    pub variable: String,
    pub relationship: String,
}
