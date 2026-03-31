use std::collections::HashSet;

use crate::classify::entry_points::AccessLevel;
use crate::sequence::analysis::{FunctionBehavior, TransitionInfo};

use super::types::*;

pub fn build_sequence_narrative(
    contract_name: &str,
    function_names: &[&str],
    behaviors: &[FunctionBehavior],
    transitions: &[TransitionInfo],
    classifications: &[(String, AccessLevel)],
) -> SequenceNarrative {
    let mut steps = Vec::new();
    let mut observations = Vec::new();

    for (i, func_name) in function_names.iter().enumerate() {
        let behavior = behaviors.iter().find(|b| b.name == *func_name);
        let access = classifications
            .iter()
            .find(|(name, _)| name == func_name)
            .map(|(_, a)| a.clone())
            .unwrap_or(AccessLevel::Public);

        let (requires, effects, ext_calls, events) = match behavior {
            Some(b) => (
                b.preconditions.clone(),
                b.state_writes.clone(),
                b.external_calls.clone(),
                b.events.clone(),
            ),
            None => (vec![], vec![], vec![], vec![]),
        };

        let mut deps = Vec::new();
        let mut seen_vars: HashSet<String> = HashSet::new();

        for prev_idx in 0..i {
            let prev_name = function_names[prev_idx];
            let transition = transitions
                .iter()
                .find(|t| t.from == prev_name && t.to == *func_name);

            if let Some(t) = transition {
                // Prefer conditions_affected (more detailed) over shared_state
                for cond in &t.conditions_affected {
                    deps.push(Dependency {
                        from_step: prev_idx,
                        variable: extract_variable_from_condition(cond),
                        relationship: cond.clone(),
                    });
                    // Track which variables already have a dependency
                    for var in &t.shared_state {
                        if cond.contains(var.as_str()) {
                            seen_vars.insert(var.clone());
                        }
                    }
                }

                // Only add shared_state deps for variables NOT already covered
                for var in &t.shared_state {
                    if seen_vars.contains(var) { continue; }
                    seen_vars.insert(var.clone());
                    deps.push(Dependency {
                        from_step: prev_idx,
                        variable: var.clone(),
                        relationship: format!(
                            "{} writes {} → {} reads/writes it",
                            prev_name, var, func_name,
                        ),
                    });
                }
            }
        }

        steps.push(SequenceStep {
            function: func_name.to_string(),
            access,
            requires,
            effects,
            external_calls: ext_calls,
            events,
            dependencies: deps,
        });
    }

    for i in 0..steps.len() {
        if steps[i].external_calls.is_empty() { continue; }
        for j in (i + 1)..steps.len() {
            if steps[j].dependencies.iter().any(|d| d.from_step == i) {
                observations.push(Observation {
                    kind: ObservationKind::SharedState,
                    description: format!(
                        "Step {} ({}) makes external calls, step {} ({}) depends on its state — cross-step interaction",
                        i + 1, steps[i].function,
                        j + 1, steps[j].function,
                    ),
                });
            }
        }
    }

    SequenceNarrative {
        contract: contract_name.to_string(),
        steps,
        observations,
    }
}

fn extract_variable_from_condition(cond: &str) -> String {
    // conditions_affected format: "deposit writes 'balances' → withdraw needs require(...)"
    if let Some(start) = cond.find('\'') {
        if let Some(end) = cond[start + 1..].find('\'') {
            return cond[start + 1..start + 1 + end].to_string();
        }
    }
    String::new()
}
