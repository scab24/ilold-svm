use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::exploration::assign_operator::AssignOperator;
use crate::journal::types::AuditJournal;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationSession {
    pub contract: String,
    pub steps: Vec<ExplorationStep>,
    pub journal: AuditJournal,
    #[serde(default)]
    pub forked_from: Option<ForkOrigin>,
    #[serde(default)]
    pub failed_calls_per_ix: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForkOrigin {
    pub scenario: String,
    pub at_step: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationStep {
    pub function: String,
    pub mutations: Vec<StateMutation>,
    #[serde(default)]
    pub flow_tree: Option<serde_json::Value>,
    #[serde(default)]
    pub trace_config: TraceConfig,
    #[serde(default)]
    pub runtime_trace: Option<serde_json::Value>,
    #[serde(default)]
    pub call_payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum MutationScope {
    #[default]
    State,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMutation {
    pub variable: String,
    pub operator: AssignOperator,
    pub value_expr: String,
    pub step_index: usize,
    #[serde(default)]
    pub via: Option<String>,
    #[serde(default)]
    pub flow_step_id: Option<usize>,
    #[serde(default)]
    pub scope: MutationScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceConfig {
    #[serde(default = "default_trace_depth")]
    pub depth: usize,
    #[serde(default)]
    pub include_reverts: bool,
    #[serde(default)]
    pub expand_set: Vec<usize>,
}

impl Default for TraceConfig {
    fn default() -> Self {
        Self {
            depth: 2,
            include_reverts: false,
            expand_set: Vec::new(),
        }
    }
}

fn default_trace_depth() -> usize { 2 }

impl ExplorationSession {
    pub fn new(contract: &str, project: &str) -> Self {
        ExplorationSession {
            contract: contract.into(),
            steps: Vec::new(),
            journal: AuditJournal::new(project, contract, ""),
            forked_from: None,
            failed_calls_per_ix: BTreeMap::new(),
        }
    }

    pub fn remove_last_step(&mut self) -> bool {
        self.steps.pop().is_some()
    }

    pub fn clear(&mut self) {
        self.steps.clear();
        self.failed_calls_per_ix.clear();
    }

    pub fn record_failed_call(&mut self, ix: &str) {
        *self
            .failed_calls_per_ix
            .entry(ix.to_string())
            .or_insert(0) += 1;
    }

    pub fn reset_scenario_local_observations(&mut self) {
        self.failed_calls_per_ix.clear();
    }

    pub fn current_sequence(&self) -> Vec<&str> {
        self.steps.iter().map(|s| s.function.as_str()).collect()
    }

    fn variable_history(&self) -> HashMap<String, Vec<&StateMutation>> {
        let mut history: HashMap<String, Vec<&StateMutation>> = HashMap::new();
        for step in &self.steps {
            for mutation in &step.mutations {
                history.entry(mutation.variable.clone())
                    .or_default()
                    .push(mutation);
            }
        }
        history
    }

    pub fn variable_summary(&self) -> Vec<VariableSummary> {
        let history = self.variable_history();
        let mut summaries: Vec<VariableSummary> = history.into_iter().map(|(var, muts)| {
            let changes: Vec<String> = muts.iter().map(|m| {
                let op_str = m.operator.as_str();
                let func = self.steps.get(m.step_index)
                    .map(|s| s.function.as_str())
                    .unwrap_or("?");
                let step_ref = match m.flow_step_id {
                    Some(id) => format!("step {}:{}", m.step_index, id),
                    None => format!("step {}", m.step_index),
                };
                let suffix = match &m.via {
                    Some(chain) => format!(" via {}", chain),
                    None => String::new(),
                };
                format!("{} {} ({}, {}{})", op_str, m.value_expr, step_ref, func, suffix)
            }).collect();

            VariableSummary { variable: var, changes }
        }).collect();

        summaries.sort_by(|a, b| a.variable.cmp(&b.variable));
        summaries
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableSummary {
    pub variable: String,
    pub changes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_session_is_empty() {
        let s = ExplorationSession::new("Staking", "myproject");
        assert_eq!(s.contract, "Staking");
        assert!(s.steps.is_empty());
        assert_eq!(s.current_sequence().len(), 0);
    }

    #[test]
    fn remove_last_step_empty() {
        let mut s = ExplorationSession::new("Staking", "myproject");
        assert!(!s.remove_last_step());
    }

    #[test]
    fn reset_scenario_local_observations_clears_failed_calls() {
        let mut s = ExplorationSession::new("Staking", "myproject");
        s.record_failed_call("stake");
        s.record_failed_call("stake");
        s.record_failed_call("unstake");
        s.steps.push(ExplorationStep {
            function: "deposit".into(),
            mutations: vec![],
            flow_tree: None,
            trace_config: TraceConfig::default(),
            runtime_trace: None,
            call_payload: None,
        });
        assert_eq!(s.failed_calls_per_ix.get("stake").copied(), Some(2));
        s.reset_scenario_local_observations();
        assert!(s.failed_calls_per_ix.is_empty());
        assert_eq!(s.steps.len(), 1);
    }

    #[test]
    fn clear_resets() {
        let mut s = ExplorationSession::new("Staking", "myproject");
        s.steps.push(ExplorationStep {
            function: "deposit".into(),
            mutations: vec![],
            flow_tree: None,
            trace_config: TraceConfig::default(),
            runtime_trace: None,
            call_payload: None,
        });
        s.steps.push(ExplorationStep {
            function: "withdraw".into(),
            mutations: vec![],
            flow_tree: None,
            trace_config: TraceConfig::default(),
            runtime_trace: None,
            call_payload: None,
        });
        assert_eq!(s.current_sequence(), vec!["deposit", "withdraw"]);
        s.clear();
        assert!(s.steps.is_empty());
    }

    #[test]
    fn variable_summary_computes() {
        let mut s = ExplorationSession::new("Staking", "myproject");
        s.steps.push(ExplorationStep {
            function: "deposit".into(),
            mutations: vec![
                StateMutation {
                    variable: "balances".into(),
                    operator: AssignOperator::AddAssign,
                    value_expr: "amount".into(),
                    step_index: 0,
                    via: None,
                    flow_step_id: None,
                    scope: MutationScope::State,
                },
                StateMutation {
                    variable: "totalStaked".into(),
                    operator: AssignOperator::AddAssign,
                    value_expr: "amount".into(),
                    step_index: 0,
                    via: None,
                    flow_step_id: None,
                    scope: MutationScope::State,
                },
            ],
            flow_tree: None,
            trace_config: TraceConfig::default(),
            runtime_trace: None,
            call_payload: None,
        });
        s.steps.push(ExplorationStep {
            function: "withdraw".into(),
            mutations: vec![
                StateMutation {
                    variable: "balances".into(),
                    operator: AssignOperator::SubAssign,
                    value_expr: "amount".into(),
                    step_index: 1,
                    via: None,
                    flow_step_id: None,
                    scope: MutationScope::State,
                },
            ],
            flow_tree: None,
            trace_config: TraceConfig::default(),
            runtime_trace: None,
            call_payload: None,
        });

        let summaries = s.variable_summary();
        assert_eq!(summaries.len(), 2);

        let balances = summaries.iter().find(|s| s.variable == "balances").unwrap();
        assert_eq!(balances.changes.len(), 2);
        assert!(balances.changes[0].contains("+= amount"));
        assert!(balances.changes[1].contains("-= amount"));
    }
}
