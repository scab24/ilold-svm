use std::collections::HashSet;

use crate::cfg::types::{BlockKind, BranchEdge, CfgGraph, CfgStatement};
use crate::classify::entry_points::{classify_function, AccessLevel};
use crate::model::contract::ContractDef;
use crate::model::function::FunctionDef;
use crate::pathtree::types::{PathTree, TerminalKind};
use crate::sequence::analysis::FunctionBehavior;

use super::types::*;

pub fn build_function_narrative(
    contract: &ContractDef,
    function: &FunctionDef,
    path_tree: &PathTree,
    cfg: &CfgGraph,
    all_behaviors: &[FunctionBehavior],
) -> FunctionNarrative {
    let access = classify_function(function, contract);

    let paths: Vec<PathNarrative> = path_tree
        .paths
        .iter()
        .map(|path| {
            let mut steps = Vec::new();

            for node in &path.nodes {
                let block = cfg
                    .node_indices()
                    .find(|&n| cfg[n].id == node.block_id)
                    .map(|n| &cfg[n]);

                let block = match block {
                    Some(b) => b,
                    None => continue,
                };

                let branch = branch_direction(&node.branch_taken);

                match block.kind {
                    BlockKind::Entry => {
                        steps.push(NarrativeStep {
                            step_type: StepType::Entry,
                            description: format!("{}()", function.name),
                            detail: None,
                            branch: None,
                        });
                    }
                    BlockKind::Return => {
                        steps.push(NarrativeStep {
                            step_type: StepType::Return,
                            description: "return".into(),
                            detail: None,
                            branch,
                        });
                    }
                    BlockKind::Revert => {
                        steps.push(NarrativeStep {
                            step_type: StepType::Revert,
                            description: "revert".into(),
                            detail: None,
                            branch,
                        });
                    }
                    _ => {
                        for stmt in &block.statements {
                            if let Some(step) = statement_to_step(stmt, branch) {
                                steps.push(step);
                            }
                        }
                    }
                }
            }

            PathNarrative {
                id: path.id,
                terminal: path.terminal,
                steps,
            }
        })
        .collect();

    let observations = generate_observations(
        &access, path_tree, cfg, all_behaviors, &function.name,
    );

    let mut writes = HashSet::new();
    let mut reads = HashSet::new();
    let mut calls = HashSet::new();
    for p in &path_tree.paths {
        for w in &p.annotations.state_writes { writes.insert(w.clone()); }
        for r in &p.annotations.state_reads { reads.insert(r.clone()); }
        for c in &p.annotations.external_calls {
            calls.insert(format!("{}.{}", c.target, c.function));
        }
    }

    FunctionNarrative {
        contract: contract.name.clone(),
        name: function.name.clone(),
        access,
        total_paths: path_tree.stats.total_paths,
        happy_paths: path_tree.stats.happy_paths,
        revert_paths: path_tree.stats.revert_paths,
        paths,
        observations,
        state_writes: writes.into_iter().collect(),
        state_reads: reads.into_iter().collect(),
        external_calls: calls.into_iter().collect(),
        modifiers: function.modifiers.iter().map(|m| m.name.clone()).collect(),
    }
}

fn statement_to_step(stmt: &CfgStatement, branch: Option<BranchDirection>) -> Option<NarrativeStep> {
    match stmt {
        CfgStatement::RequireCheck { condition, .. } => Some(NarrativeStep {
            step_type: StepType::Condition,
            description: format!("require({})", condition),
            detail: None,
            branch,
        }),
        CfgStatement::AssertCheck { condition, .. } => Some(NarrativeStep {
            step_type: StepType::Condition,
            description: format!("assert({})", condition),
            detail: None,
            branch,
        }),
        CfgStatement::ExternalCall { target, function, .. } => Some(NarrativeStep {
            step_type: StepType::ExternalCall,
            description: format!("{}.{}()", target, function),
            detail: None,
            branch: None,
        }),
        CfgStatement::InternalCall { function, .. } => Some(NarrativeStep {
            step_type: StepType::InternalCall,
            description: format!("{}()", function),
            detail: None,
            branch: None,
        }),
        CfgStatement::StateWrite { variable, .. } => Some(NarrativeStep {
            step_type: StepType::StateWrite,
            description: variable.clone(),
            detail: None,
            branch: None,
        }),
        CfgStatement::StateRead { variable, .. } => Some(NarrativeStep {
            step_type: StepType::StateRead,
            description: variable.clone(),
            detail: None,
            branch: None,
        }),
        CfgStatement::EthTransfer { to, .. } => Some(NarrativeStep {
            step_type: StepType::EthTransfer,
            description: format!("transfer ETH to {}", to),
            detail: None,
            branch: None,
        }),
        CfgStatement::EmitEvent { event, .. } => Some(NarrativeStep {
            step_type: StepType::Event,
            description: format!("emit {}", event),
            detail: None,
            branch: None,
        }),
        CfgStatement::AssemblyBlock { .. } => Some(NarrativeStep {
            step_type: StepType::Assembly,
            description: "assembly block".into(),
            detail: None,
            branch: None,
        }),
        CfgStatement::Assignment { .. } => None,
    }
}

fn branch_direction(edge: &Option<BranchEdge>) -> Option<BranchDirection> {
    match edge {
        Some(BranchEdge::ConditionalTrue { .. }) => Some(BranchDirection::True),
        Some(BranchEdge::ConditionalFalse { .. }) => Some(BranchDirection::False),
        _ => None,
    }
}

fn check_cei_violation(
    path: &crate::pathtree::types::ExecutionPath,
    cfg: &CfgGraph,
) -> Option<(String, String)> {
    let mut seen_external_call: Option<String> = None;

    for node in &path.nodes {
        let block = cfg
            .node_indices()
            .find(|&n| cfg[n].id == node.block_id)
            .map(|n| &cfg[n]);

        let block = match block {
            Some(b) => b,
            None => continue,
        };

        for stmt in &block.statements {
            match stmt {
                CfgStatement::ExternalCall { target, function, .. } => {
                    if seen_external_call.is_none() {
                        seen_external_call = Some(format!("{}.{}", target, function));
                    }
                }
                CfgStatement::StateWrite { variable, .. } => {
                    if let Some(call) = &seen_external_call {
                        return Some((call.clone(), variable.clone()));
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn generate_observations(
    access: &AccessLevel,
    path_tree: &PathTree,
    cfg: &CfgGraph,
    all_behaviors: &[FunctionBehavior],
    function_name: &str,
) -> Vec<Observation> {
    let mut obs = Vec::new();

    for path in &path_tree.paths {
        if path.terminal == TerminalKind::Revert { continue; }

        if let Some((call, write)) = check_cei_violation(path, cfg) {
            obs.push(Observation {
                kind: ObservationKind::CeiViolation,
                description: format!(
                    "Path #{}: {} called BEFORE writing {}",
                    path.id, call, write,
                ),
            });
            break;
        }
    }

    let my_writes: HashSet<&str> = path_tree
        .paths
        .iter()
        .flat_map(|p| p.annotations.state_writes.iter().map(|s| s.as_str()))
        .collect();

    for behavior in all_behaviors {
        if behavior.name == function_name { continue; }
        let shared: Vec<&str> = behavior
            .state_reads
            .iter()
            .filter(|r| my_writes.contains(r.as_str()))
            .map(|s| s.as_str())
            .collect();
        if !shared.is_empty() {
            obs.push(Observation {
                kind: ObservationKind::SharedState,
                description: format!(
                    "{} reads {} which this function writes",
                    behavior.name,
                    shared.join(", "),
                ),
            });
        }
    }

    if access.is_unrestricted() && !my_writes.is_empty() {
        obs.push(Observation {
            kind: ObservationKind::NoAccessControl,
            description: format!(
                "Public function writes state ({}) without access control",
                my_writes.into_iter().collect::<Vec<_>>().join(", "),
            ),
        });
    }

    obs
}
