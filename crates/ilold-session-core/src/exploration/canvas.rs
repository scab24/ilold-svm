use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::access::AccessLevel;
use super::scenario::ScenarioEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CanvasPatch {
    AddNode {
        scenario: String,
        function: String,
        access: AccessLevel,
        step_index: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        runtime: Option<RuntimeMeta>,
    },
    RemoveLastNode { scenario: String },
    ClearAll { scenario: String },
    Highlight { scenario: String, function: String },
    ScenarioEvent(ScenarioEvent),
    SolanaUsersChanged { scenario: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeMeta {
    pub compute_units: u64,
    pub diffs_count: usize,
    pub logs_excerpt: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<Value>,
}
