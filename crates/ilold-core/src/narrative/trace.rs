// FlowTree builder: walks a function CFG and produces a temporal execution
// trace with inlined internal calls and per-arm branch walks.

use std::collections::{HashMap, HashSet};

use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::{Deserialize, Serialize};

use crate::cfg::types::{BlockKind, BranchEdge, CfgGraph, CfgStatement};
use crate::model::contract::ContractDef;
use crate::model::expression::AssignOperator;
use crate::model::function::FunctionDef;
use crate::model::project::Project;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowTree {
    pub contract: String,
    pub function: String,
    pub signature: String,
    pub modifiers: Vec<String>,
    pub max_depth: usize,
    pub root: FlowNode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowNode {
    pub step_id: usize,
    pub depth: usize,
    pub kind: FlowKind,
    #[serde(default)]
    pub from_modifier: Option<String>,
    pub children: Vec<FlowNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlowKind {
    Entry {
        signature: String,
    },
    Require {
        condition: String,
        message: Option<String>,
    },
    Assert {
        condition: String,
    },
    Write {
        target: String,
        value: String,
        op: AssignOperator,
    },
    StateWrite {
        variable: String,
    },
    StateRead {
        variable: String,
    },
    InternalCall {
        function: String,
        origin: String,
        depth_limited: bool,
        ops_count: usize,
    },
    ExternalCall {
        target: String,
        function: String,
    },
    EmitEvent {
        name: String,
    },
    BranchTrue {
        condition: String,
    },
    BranchFalse {
        condition: String,
    },
    LoopHeader {
        kind: String,
    },
    Return,
    Revert {
        reason: Option<String>,
    },
    EthTransfer {
        to: String,
    },
    AssemblyBlock,
    DepthLimit,
}

pub struct FlowConfig {
    pub max_depth: usize,
    pub include_reverts: bool,
}

impl Default for FlowConfig {
    fn default() -> Self {
        FlowConfig {
            max_depth: 2,
            include_reverts: false,
        }
    }
}

pub fn build_flow_tree(
    contract: &ContractDef,
    function: &FunctionDef,
    cfg: &CfgGraph,
    project: &Project,
    all_cfgs: &HashMap<(String, String), CfgGraph>,
    config: &FlowConfig,
) -> FlowTree {
    let signature = build_signature(function);

    let mut counter: usize = 0;
    let mut root = FlowNode {
        step_id: next_id(&mut counter),
        depth: 0,
        kind: FlowKind::Entry { signature: signature.clone() },
        from_modifier: None,
        children: Vec::new(),
    };

    let mut visited_calls: HashSet<(String, String)> = HashSet::new();
    visited_calls.insert((contract.name.clone(), function.name.clone()));

    walk_cfg(
        cfg,
        contract,
        project,
        all_cfgs,
        config,
        0,
        &mut counter,
        &mut visited_calls,
        &mut root.children,
    );

    FlowTree {
        contract: contract.name.clone(),
        function: function.name.clone(),
        signature,
        modifiers: function.modifiers.iter().map(|m| m.name.clone()).collect(),
        max_depth: config.max_depth,
        root,
    }
}

fn next_id(counter: &mut usize) -> usize {
    let id = *counter;
    *counter += 1;
    id
}

fn build_signature(function: &FunctionDef) -> String {
    let params: Vec<String> = function
        .params
        .iter()
        .map(|p| format!("{} {}", p.type_name, p.name))
        .collect();
    format!("{}({})", function.name, params.join(", "))
}

fn walk_cfg(
    cfg: &CfgGraph,
    contract: &ContractDef,
    project: &Project,
    all_cfgs: &HashMap<(String, String), CfgGraph>,
    config: &FlowConfig,
    depth: usize,
    counter: &mut usize,
    visited_calls: &mut HashSet<(String, String)>,
    out: &mut Vec<FlowNode>,
) {
    let entry = match cfg
        .node_indices()
        .find(|&n| cfg[n].kind == BlockKind::Entry)
    {
        Some(n) => n,
        None => return,
    };

    let mut visited_blocks: HashSet<petgraph::stable_graph::NodeIndex> = HashSet::new();
    walk_block(
        cfg,
        entry,
        contract,
        project,
        all_cfgs,
        config,
        depth,
        counter,
        visited_calls,
        &mut visited_blocks,
        out,
    );
}

#[allow(clippy::too_many_arguments)]
fn walk_block(
    cfg: &CfgGraph,
    node: petgraph::stable_graph::NodeIndex,
    contract: &ContractDef,
    project: &Project,
    all_cfgs: &HashMap<(String, String), CfgGraph>,
    config: &FlowConfig,
    depth: usize,
    counter: &mut usize,
    visited_calls: &mut HashSet<(String, String)>,
    visited_blocks: &mut HashSet<petgraph::stable_graph::NodeIndex>,
    out: &mut Vec<FlowNode>,
) {
    if !visited_blocks.insert(node) {
        return;
    }

    let block = &cfg[node];

    match block.kind {
        BlockKind::Return => {
            out.push(FlowNode {
                step_id: next_id(counter),
                depth,
                kind: FlowKind::Return,
                from_modifier: None,
                children: Vec::new(),
            });
            return;
        }
        BlockKind::Revert => {
            if config.include_reverts {
                out.push(FlowNode {
                    step_id: next_id(counter),
                    depth,
                    kind: FlowKind::Revert { reason: None },
                    from_modifier: None,
                    children: Vec::new(),
                });
            }
            return;
        }
        _ => {}
    }

    for stmt in &block.statements {
        if let Some(node) = stmt_to_flow_node(
            stmt,
            depth,
            counter,
            contract,
            project,
            all_cfgs,
            config,
            visited_calls,
        ) {
            out.push(node);
        }
    }

    let edges: Vec<(petgraph::stable_graph::NodeIndex, BranchEdge)> = cfg
        .edges_directed(node, Direction::Outgoing)
        .map(|e| (e.target(), e.weight().clone()))
        .collect();

    if edges.is_empty() {
        return;
    }

    if edges.len() == 1 {
        walk_block(
            cfg,
            edges[0].0,
            contract,
            project,
            all_cfgs,
            config,
            depth,
            counter,
            visited_calls,
            visited_blocks,
            out,
        );
        return;
    }

    // Drop arms going to Revert blocks (require/assert/if-then-revert).
    let mut useful: Vec<(petgraph::stable_graph::NodeIndex, BranchEdge)> = Vec::new();
    for (target, edge) in edges {
        if cfg[target].kind == BlockKind::Revert && !config.include_reverts {
            continue;
        }
        useful.push((target, edge));
    }

    if useful.is_empty() {
        return;
    }

    // One useful arm: collapse require/assert into linear walk.
    if useful.len() == 1 {
        let (target, _edge) = useful.into_iter().next().unwrap();
        walk_block(
            cfg,
            target,
            contract,
            project,
            all_cfgs,
            config,
            depth,
            counter,
            visited_calls,
            visited_blocks,
            out,
        );
        return;
    }

    // Real if/else: clone visited_blocks per arm so each branch is complete.
    // Post-merge code is duplicated under each arm.
    for (target, edge) in useful {
        let mut arm_visited = visited_blocks.clone();
        let mut children = Vec::new();
        walk_block(
            cfg,
            target,
            contract,
            project,
            all_cfgs,
            config,
            depth + 1,
            counter,
            visited_calls,
            &mut arm_visited,
            &mut children,
        );

        if children.is_empty() {
            continue;
        }

        let kind = match edge {
            BranchEdge::ConditionalTrue { condition } => FlowKind::BranchTrue { condition },
            BranchEdge::ConditionalFalse { condition } => FlowKind::BranchFalse { condition },
            BranchEdge::ExternalCallSuccess => FlowKind::BranchTrue {
                condition: "<call success>".to_string(),
            },
            BranchEdge::ExternalCallFailure => FlowKind::BranchFalse {
                condition: "<call failure>".to_string(),
            },
            BranchEdge::CatchClause { kind: catch_kind } => FlowKind::BranchTrue {
                condition: format!("catch {}", catch_kind),
            },
            BranchEdge::LoopBack => FlowKind::LoopHeader { kind: "back".to_string() },
            BranchEdge::LoopExit => FlowKind::LoopHeader { kind: "exit".to_string() },
            BranchEdge::Unconditional => {
                out.extend(children);
                continue;
            }
        };
        out.push(FlowNode {
            step_id: next_id(counter),
            depth,
            kind,
            from_modifier: None,
            children,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn stmt_to_flow_node(
    stmt: &CfgStatement,
    depth: usize,
    counter: &mut usize,
    contract: &ContractDef,
    project: &Project,
    all_cfgs: &HashMap<(String, String), CfgGraph>,
    config: &FlowConfig,
    visited_calls: &mut HashSet<(String, String)>,
) -> Option<FlowNode> {
    match stmt {
        CfgStatement::Assignment { target, value, operator, from_modifier, .. } => {
            Some(FlowNode {
                step_id: next_id(counter),
                depth,
                kind: FlowKind::Write {
                    target: target.clone(),
                    value: value.clone(),
                    op: *operator,
                },
                from_modifier: from_modifier.clone(),
                children: Vec::new(),
            })
        }
        CfgStatement::RequireCheck { condition, message, from_modifier, .. } => {
            Some(FlowNode {
                step_id: next_id(counter),
                depth,
                kind: FlowKind::Require {
                    condition: condition.clone(),
                    message: message.clone(),
                },
                from_modifier: from_modifier.clone(),
                children: Vec::new(),
            })
        }
        CfgStatement::AssertCheck { condition, from_modifier, .. } => {
            Some(FlowNode {
                step_id: next_id(counter),
                depth,
                kind: FlowKind::Assert { condition: condition.clone() },
                from_modifier: from_modifier.clone(),
                children: Vec::new(),
            })
        }
        CfgStatement::InternalCall { function, from_modifier, .. } => {
            Some(build_internal_call_node(
                function,
                from_modifier.clone(),
                depth,
                counter,
                contract,
                project,
                all_cfgs,
                config,
                visited_calls,
            ))
        }
        CfgStatement::ExternalCall { target, function, from_modifier, .. } => {
            Some(FlowNode {
                step_id: next_id(counter),
                depth,
                kind: FlowKind::ExternalCall {
                    target: target.clone(),
                    function: function.clone(),
                },
                from_modifier: from_modifier.clone(),
                children: Vec::new(),
            })
        }
        CfgStatement::EmitEvent { event, from_modifier, .. } => {
            Some(FlowNode {
                step_id: next_id(counter),
                depth,
                kind: FlowKind::EmitEvent { name: event.clone() },
                from_modifier: from_modifier.clone(),
                children: Vec::new(),
            })
        }
        CfgStatement::StateWrite { variable, from_modifier, .. } => {
            Some(FlowNode {
                step_id: next_id(counter),
                depth,
                kind: FlowKind::StateWrite { variable: variable.clone() },
                from_modifier: from_modifier.clone(),
                children: Vec::new(),
            })
        }
        CfgStatement::StateRead { variable, from_modifier, .. } => {
            Some(FlowNode {
                step_id: next_id(counter),
                depth,
                kind: FlowKind::StateRead { variable: variable.clone() },
                from_modifier: from_modifier.clone(),
                children: Vec::new(),
            })
        }
        CfgStatement::EthTransfer { to, from_modifier, .. } => {
            Some(FlowNode {
                step_id: next_id(counter),
                depth,
                kind: FlowKind::EthTransfer { to: to.clone() },
                from_modifier: from_modifier.clone(),
                children: Vec::new(),
            })
        }
        CfgStatement::AssemblyBlock { from_modifier, .. } => {
            Some(FlowNode {
                step_id: next_id(counter),
                depth,
                kind: FlowKind::AssemblyBlock,
                from_modifier: from_modifier.clone(),
                children: Vec::new(),
            })
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_internal_call_node(
    callee_name: &str,
    from_modifier: Option<String>,
    depth: usize,
    counter: &mut usize,
    contract: &ContractDef,
    project: &Project,
    all_cfgs: &HashMap<(String, String), CfgGraph>,
    config: &FlowConfig,
    visited_calls: &mut HashSet<(String, String)>,
) -> FlowNode {
    let resolved = resolve_callee_cfg(callee_name, contract, project, all_cfgs);

    let (owning_name, callee_cfg) = match resolved {
        Some(x) => x,
        None => {
            return FlowNode {
                step_id: next_id(counter),
                depth,
                kind: FlowKind::InternalCall {
                    function: callee_name.to_string(),
                    origin: String::new(),
                    depth_limited: false,
                    ops_count: 0,
                },
                from_modifier,
                children: Vec::new(),
            };
        }
    };

    let call_key = (owning_name.clone(), callee_name.to_string());
    let already_visited = visited_calls.contains(&call_key);
    let depth_exhausted = depth + 1 > config.max_depth;

    if already_visited || depth_exhausted {
        let ops_count = count_statements(callee_cfg);
        return FlowNode {
            step_id: next_id(counter),
            depth,
            kind: FlowKind::InternalCall {
                function: callee_name.to_string(),
                origin: owning_name,
                depth_limited: true,
                ops_count,
            },
            from_modifier,
            children: Vec::new(),
        };
    }

    visited_calls.insert(call_key.clone());

    let owning_contract = project
        .contracts
        .iter()
        .find(|c| c.name == owning_name)
        .unwrap_or(contract);

    let mut children = Vec::new();
    walk_cfg(
        callee_cfg,
        owning_contract,
        project,
        all_cfgs,
        config,
        depth + 1,
        counter,
        visited_calls,
        &mut children,
    );

    // Pop the call stack so sibling calls to the same function still inline.
    visited_calls.remove(&call_key);

    let ops_count = children.len();

    FlowNode {
        step_id: next_id(counter),
        depth,
        kind: FlowKind::InternalCall {
            function: callee_name.to_string(),
            origin: owning_name,
            depth_limited: false,
            ops_count,
        },
        from_modifier,
        children,
    }
}

fn resolve_callee_cfg<'a>(
    callee: &str,
    starting_contract: &ContractDef,
    project: &Project,
    all_cfgs: &'a HashMap<(String, String), CfgGraph>,
) -> Option<(String, &'a CfgGraph)> {
    let key = (starting_contract.name.clone(), callee.to_string());
    if let Some(cfg) = all_cfgs.get(&key) {
        return Some((starting_contract.name.clone(), cfg));
    }

    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: Vec<String> = starting_contract.inherits.clone();

    while let Some(parent_name) = queue.pop() {
        if !visited.insert(parent_name.clone()) {
            continue;
        }

        let key = (parent_name.clone(), callee.to_string());
        if let Some(cfg) = all_cfgs.get(&key) {
            return Some((parent_name, cfg));
        }

        if let Some(parent_idx) = project.contract_index.get(&parent_name) {
            let parent = &project.contracts[*parent_idx];
            for grand in &parent.inherits {
                queue.push(grand.clone());
            }
        }
    }

    None
}

fn count_statements(cfg: &CfgGraph) -> usize {
    cfg.node_indices()
        .map(|n| cfg[n].statements.len())
        .sum()
}
