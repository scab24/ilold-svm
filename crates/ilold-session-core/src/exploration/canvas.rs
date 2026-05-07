use serde::{Deserialize, Serialize};

use super::access::AccessLevel;
use super::scenario::ScenarioEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CanvasPatch {
    AddNode { scenario: String, function: String, access: AccessLevel, step_index: usize },
    RemoveLastNode { scenario: String },
    ClearAll { scenario: String },
    Highlight { scenario: String, function: String },
    ScenarioEvent(ScenarioEvent),
    SolanaUsersChanged { scenario: String },
}
