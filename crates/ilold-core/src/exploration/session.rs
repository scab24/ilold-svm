use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::cfg::types::{CfgGraph, CfgStatement};
use crate::journal::types::AuditJournal;
use crate::model::common::StateVar;
use crate::model::expression::AssignOperator;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationSession {
    pub contract: String,
    pub steps: Vec<ExplorationStep>,
    pub journal: AuditJournal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationStep {
    pub function: String,
    pub mutations: Vec<StateMutation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMutation {
    pub variable: String,
    pub operator: AssignOperator,
    pub value_expr: String,
    pub step_index: usize,
}

impl ExplorationSession {
    pub fn new(contract: &str, project: &str) -> Self {
        ExplorationSession {
            contract: contract.into(),
            steps: Vec::new(),
            journal: AuditJournal::new(project, contract, ""),
        }
    }

    pub fn add_step(
        &mut self,
        function: &str,
        cfg: &CfgGraph,
        state_vars: &[StateVar],
        timestamp: &str,
    ) -> &ExplorationStep {
        let step_index = self.steps.len();
        let mutations = extract_mutations(cfg, state_vars, step_index);

        self.steps.push(ExplorationStep {
            function: function.into(),
            mutations,
        });

        self.journal.record(crate::journal::types::JournalEntry::SequenceExplored {
            steps: self.steps.iter().map(|s| s.function.clone()).collect(),
            timestamp: timestamp.into(),
        });

        self.steps.last().unwrap()
    }

    pub fn remove_last_step(&mut self) -> bool {
        self.steps.pop().is_some()
    }

    pub fn clear(&mut self) {
        self.steps.clear();
    }

    pub fn current_sequence(&self) -> Vec<&str> {
        self.steps.iter().map(|s| s.function.as_str()).collect()
    }

    pub fn variable_history(&self) -> HashMap<String, Vec<&StateMutation>> {
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
                let op_str = match m.operator {
                    AssignOperator::AddAssign => "+",
                    AssignOperator::SubAssign => "-",
                    AssignOperator::Assign => "=",
                    _ => "?",
                };
                let func = self.steps.get(m.step_index)
                    .map(|s| s.function.as_str())
                    .unwrap_or("?");
                format!("{}{} (step {}, {})", op_str, m.value_expr, m.step_index + 1, func)
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

fn extract_mutations(
    cfg: &CfgGraph,
    state_vars: &[StateVar],
    step_index: usize,
) -> Vec<StateMutation> {
    let mut mutations = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for node_idx in cfg.node_indices() {
        let block = &cfg[node_idx];
        for stmt in &block.statements {
            match stmt {
                CfgStatement::Assignment { target, operator, value, .. } => {
                    let base = target.split('[').next().unwrap_or(target);
                    let base = base.split('.').next().unwrap_or(base);
                    if state_vars.iter().any(|sv| sv.name == base) {
                        let key = (target.clone(), *operator);
                        if seen.insert(key) {
                            mutations.push(StateMutation {
                                variable: target.clone(),
                                operator: *operator,
                                value_expr: value.clone(),
                                step_index,
                            });
                        }
                    }
                }
                CfgStatement::StateWrite { variable, .. } => {
                    let key = (variable.clone(), AssignOperator::Assign);
                    if seen.insert(key) {
                        mutations.push(StateMutation {
                            variable: variable.clone(),
                            operator: AssignOperator::Assign,
                            value_expr: String::new(),
                            step_index,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    mutations
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_state_var(name: &str) -> StateVar {
        StateVar {
            name: name.into(),
            type_name: "uint256".into(),
            visibility: crate::model::function::Visibility::Public,
            is_constant: false,
            is_immutable: false,
            initial_value: None,
            span: crate::model::common::SourceSpan {
                file_index: 0, start_line: 0, start_col: 0, end_line: 0, end_col: 0,
            },
        }
    }

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
    fn clear_resets() {
        let mut s = ExplorationSession::new("Staking", "myproject");
        s.steps.push(ExplorationStep {
            function: "deposit".into(),
            mutations: vec![],
        });
        s.steps.push(ExplorationStep {
            function: "withdraw".into(),
            mutations: vec![],
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
                },
                StateMutation {
                    variable: "totalStaked".into(),
                    operator: AssignOperator::AddAssign,
                    value_expr: "amount".into(),
                    step_index: 0,
                },
            ],
        });
        s.steps.push(ExplorationStep {
            function: "withdraw".into(),
            mutations: vec![
                StateMutation {
                    variable: "balances".into(),
                    operator: AssignOperator::SubAssign,
                    value_expr: "amount".into(),
                    step_index: 1,
                },
            ],
        });

        let summaries = s.variable_summary();
        assert_eq!(summaries.len(), 2);

        let balances = summaries.iter().find(|s| s.variable == "balances").unwrap();
        assert_eq!(balances.changes.len(), 2);
        assert!(balances.changes[0].contains("+amount"));
        assert!(balances.changes[1].contains("-amount"));
    }
}
