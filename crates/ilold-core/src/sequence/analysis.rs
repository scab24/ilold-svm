use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::pathtree::types::{PathTree, TerminalKind};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionBehavior {
    pub name: String,
    pub preconditions: Vec<String>,
    pub state_writes: Vec<String>,
    pub state_reads: Vec<String>,
    pub external_calls: Vec<String>,
    pub events: Vec<String>,
    pub can_revert: bool,
    pub always_reverts: bool,
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionInfo {
    pub from: String,
    pub to: String,
    pub shared_state: Vec<String>,
    pub conditions_affected: Vec<String>,
    pub has_external_in_from: bool,
    pub has_external_in_to: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceAnalysis {
    pub functions: Vec<FunctionBehavior>,
    pub transitions: Vec<TransitionInfo>,
}

pub fn analyze_sequences(
    path_trees: &HashMap<(String, String), PathTree>,
    contract_name: &str,
) -> SequenceAnalysis {
    let mut behaviors: Vec<FunctionBehavior> = Vec::new();

    for ((c, f), pt) in path_trees {
        if c != contract_name || f.is_empty() { continue; }

        let mut preconditions = HashSet::new();
        let mut state_writes = HashSet::new();
        let mut state_reads = HashSet::new();
        let mut external_calls = HashSet::new();
        let mut events = HashSet::new();
        let mut has_return = false;
        let mut has_revert = false;

        for path in &pt.paths {
            for check in &path.annotations.require_checks {
                preconditions.insert(check.clone());
            }
            for write in &path.annotations.state_writes {
                let base = write.split('[').next().unwrap_or(write).to_string();
                state_writes.insert(base);
            }
            for read in &path.annotations.state_reads {
                state_reads.insert(read.clone());
            }
            for call in &path.annotations.external_calls {
                external_calls.insert(format!("{}.{}", call.target, call.function));
            }
            for event in &path.annotations.events_emitted {
                events.insert(event.clone());
            }
            match path.terminal {
                TerminalKind::Return => has_return = true,
                TerminalKind::Revert => has_revert = true,
                _ => {}
            }
        }

        let read_only = state_writes.is_empty() && external_calls.is_empty();

        behaviors.push(FunctionBehavior {
            name: f.clone(),
            preconditions: preconditions.into_iter().collect(),
            state_writes: state_writes.into_iter().collect(),
            state_reads: state_reads.into_iter().collect(),
            external_calls: external_calls.into_iter().collect(),
            events: events.into_iter().collect(),
            can_revert: has_revert,
            always_reverts: !has_return && has_revert,
            read_only,
        });
    }

    // For each pair A→B, describe what conditions connect them
    let mut transitions = Vec::new();

    for a in &behaviors {
        for b in &behaviors {
            let shared: Vec<String> = a.state_writes.iter()
                .filter(|w| b.state_reads.contains(*w) || b.state_writes.contains(*w))
                .cloned()
                .collect();

            let mut conditions_affected: Vec<String> = Vec::new();
            for w in &a.state_writes {
                for p in &b.preconditions {
                    if p.contains(w.as_str()) {
                        conditions_affected.push(format!("{} writes '{}' → {} needs require({})", a.name, w, b.name, p));
                    }
                }
            }

            // Only include transitions that have something interesting
            if !shared.is_empty() || !conditions_affected.is_empty() || !a.external_calls.is_empty() || !b.external_calls.is_empty() {
                transitions.push(TransitionInfo {
                    from: a.name.clone(),
                    to: b.name.clone(),
                    shared_state: shared,
                    conditions_affected,
                    has_external_in_from: !a.external_calls.is_empty(),
                    has_external_in_to: !b.external_calls.is_empty(),
                });
            }
        }
    }

    SequenceAnalysis {
        functions: behaviors,
        transitions,
    }
}
