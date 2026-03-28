use std::collections::HashMap;

use petgraph::stable_graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::Direction;

use crate::cfg::types::*;
use crate::model::common::StateVar;

use super::config::PruningConfig;
use super::types::*;

/// One item on the DFS stack. Represents a path being explored.
struct WalkState {
    node: NodeIndex,
    path: Vec<PathNode>,
    annotations: PathAnnotations,
    depth: usize,
    /// How many times each LoopCondition block has been visited in THIS path.
    /// Each path gets its own copy so loops don't interfere between paths.
    loop_counts: HashMap<usize, usize>,
    /// The edge that led to this node (for recording in PathNode).
    edge_taken: Option<BranchEdge>,
}

pub fn build_path_tree(
    cfg: &CfgGraph,
    contract_name: &str,
    function_name: &str,
    state_vars: &[StateVar],
    config: &PruningConfig,
) -> PathTree {
    let mut paths: Vec<ExecutionPath> = Vec::new();
    let mut paths_pruned: usize = 0;
    let mut max_depth_reached: usize = 0;
    let mut next_path_id: usize = 0;

    // Find the Entry node
    let entry = match cfg.node_indices().find(|&n| cfg[n].kind == BlockKind::Entry) {
        Some(n) => n,
        None => {
            // No entry = interface function with no body
            return PathTree {
                contract: contract_name.to_string(),
                function: function_name.to_string(),
                paths: Vec::new(),
                stats: PathTreeStats {
                    total_paths: 0,
                    paths_pruned: 0,
                    max_depth_reached: 0,
                    revert_paths: 0,
                    happy_paths: 0,
                },
            };
        }
    };

    // DFS stack — each item is a path being explored
    let mut stack: Vec<WalkState> = vec![WalkState {
        node: entry,
        path: Vec::new(),
        annotations: PathAnnotations::default(),
        depth: 0,
        loop_counts: HashMap::new(),
        edge_taken: None,
    }];

    while let Some(state) = stack.pop() {
        // Budget check: stop if we've found enough paths
        if paths.len() >= config.max_paths {
            paths_pruned += 1;
            continue;
        }

        let block = &cfg[state.node];
        let mut path = state.path;
        let mut annotations = state.annotations;
        let mut loop_counts = state.loop_counts;
        let depth = state.depth;

        // Track max depth
        if depth > max_depth_reached {
            max_depth_reached = depth;
        }

        // Pruning: max depth
        if depth > config.max_depth {
            paths_pruned += 1;
            path.push(PathNode {
                block_id: block.id,
                block_kind: block.kind,
                branch_taken: state.edge_taken,
            });
            let id = next_path_id;
            next_path_id += 1;
            paths.push(ExecutionPath {
                id,
                nodes: path,
                terminal: TerminalKind::DepthCutoff,
                annotations,
                depth,
            });
            continue;
        }

        // Pruning: loop detection
        if block.kind == BlockKind::LoopCondition {
            let count = loop_counts.entry(block.id).or_insert(0);
            *count += 1;
            if *count > config.max_loop_unroll {
                paths_pruned += 1;
                path.push(PathNode {
                    block_id: block.id,
                    block_kind: block.kind,
                    branch_taken: state.edge_taken,
                });
                let id = next_path_id;
                next_path_id += 1;
                paths.push(ExecutionPath {
                    id,
                    nodes: path,
                    terminal: TerminalKind::LoopCutoff,
                    annotations,
                    depth,
                });
                continue;
            }
        }

        // Add this block to the path
        path.push(PathNode {
            block_id: block.id,
            block_kind: block.kind,
            branch_taken: state.edge_taken,
        });

        // Collect annotations from this block's statements
        collect_annotations(&block.statements, state_vars, &mut annotations);

        // Terminal check
        match block.kind {
            BlockKind::Return => {
                let id = next_path_id;
                next_path_id += 1;
                paths.push(ExecutionPath {
                    id,
                    nodes: path,
                    terminal: TerminalKind::Return,
                    annotations,
                    depth,
                });
                continue;
            }
            BlockKind::Revert => {
                let id = next_path_id;
                next_path_id += 1;
                paths.push(ExecutionPath {
                    id,
                    nodes: path,
                    terminal: TerminalKind::Revert,
                    annotations,
                    depth,
                });
                continue;
            }
            _ => {}
        }

        // Get outgoing edges and push successors onto stack
        let edges: Vec<_> = cfg
            .edges_directed(state.node, Direction::Outgoing)
            .map(|e| (e.target(), e.weight().clone()))
            .collect();

        if edges.is_empty() {
            // Entry with no edges = interface/abstract function (no body) → skip
            if block.kind == BlockKind::Entry && block.statements.is_empty() {
                continue;
            }
            // Dead end in a normal block → treat as implicit return
            let id = next_path_id;
            next_path_id += 1;
            paths.push(ExecutionPath {
                id,
                nodes: path,
                terminal: TerminalKind::Return,
                annotations,
                depth,
            });
            continue;
        }

        // For each outgoing edge, clone the state and push
        for (i, (target, edge)) in edges.into_iter().enumerate() {
            let (p, a, lc) = if i == 0 {
                // First edge reuses the original (avoids one clone)
                (path.clone(), annotations.clone(), loop_counts.clone())
            } else {
                (path.clone(), annotations.clone(), loop_counts.clone())
            };
            stack.push(WalkState {
                node: target,
                path: p,
                annotations: a,
                depth: depth + 1,
                loop_counts: lc,
                edge_taken: Some(edge),
            });
        }
    }

    let revert_paths = paths.iter().filter(|p| p.terminal == TerminalKind::Revert).count();
    let happy_paths = paths.iter().filter(|p| p.terminal == TerminalKind::Return).count();

    PathTree {
        contract: contract_name.to_string(),
        function: function_name.to_string(),
        paths,
        stats: PathTreeStats {
            total_paths: next_path_id,
            paths_pruned,
            max_depth_reached,
            revert_paths,
            happy_paths,
        },
    }
}

/// Scan a block's statements and accumulate annotations.
fn collect_annotations(
    statements: &[CfgStatement],
    state_vars: &[StateVar],
    annotations: &mut PathAnnotations,
) {
    for stmt in statements {
        match stmt {
            CfgStatement::ExternalCall { target, function, .. } => {
                annotations.external_calls.push(ExternalCallInfo {
                    target: target.clone(),
                    function: function.clone(),
                });
            }
            CfgStatement::InternalCall { function, .. } => {
                annotations.internal_calls.push(function.clone());
            }
            CfgStatement::EmitEvent { event, .. } => {
                annotations.events_emitted.push(event.clone());
            }
            CfgStatement::EthTransfer { to, .. } => {
                annotations.eth_transfers.push(to.clone());
            }
            CfgStatement::RequireCheck { condition, .. } => {
                annotations.require_checks.push(condition.clone());
                for sv in state_vars {
                    if condition.contains(&sv.name) && !annotations.state_reads.contains(&sv.name) {
                        annotations.state_reads.push(sv.name.clone());
                    }
                }
            }
            CfgStatement::AssertCheck { condition, .. } => {
                annotations.require_checks.push(format!("assert: {condition}"));
            }
            CfgStatement::AssemblyBlock { .. } => {
                annotations.has_assembly = true;
            }
            CfgStatement::Assignment { target, value, .. } => {
                let base_name = target.split('[').next().unwrap_or(target);
                let base_name = base_name.split('.').next().unwrap_or(base_name);
                if state_vars.iter().any(|sv| sv.name == base_name) {
                    annotations.state_writes.push(target.clone());
                }
                // Detect state reads in the value expression and target
                for sv in state_vars {
                    if value.contains(&sv.name) && !annotations.state_reads.contains(&sv.name) {
                        annotations.state_reads.push(sv.name.clone());
                    }
                }
            }
            CfgStatement::StateWrite { variable, .. } => {
                annotations.state_writes.push(variable.clone());
            }
            CfgStatement::StateRead { variable, .. } => {
                annotations.state_reads.push(variable.clone());
            }
        }
    }
}
