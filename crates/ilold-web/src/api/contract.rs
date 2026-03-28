use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use ilold_core::pathtree::types::PathTree;
use ilold_core::sequence::types::SequenceTree;

use crate::state::AppState;

// ============================================================================
// Contract detail
// ============================================================================

#[derive(Serialize)]
pub struct ContractDetail {
    pub name: String,
    pub kind: String,
    pub inherits: Vec<String>,
    pub functions: Vec<FunctionSummary>,
    pub state_vars: Vec<StateVarSummary>,
}

#[derive(Serialize)]
pub struct FunctionSummary {
    pub name: String,
    pub kind: String,
    pub visibility: String,
    pub mutability: String,
    pub params: Vec<ParamSummary>,
    pub path_count: usize,
    pub happy_paths: usize,
    pub revert_paths: usize,
}

#[derive(Serialize)]
pub struct ParamSummary {
    pub name: String,
    pub type_name: String,
}

#[derive(Serialize)]
pub struct StateVarSummary {
    pub name: String,
    pub type_name: String,
    pub visibility: String,
    pub is_constant: bool,
    pub is_immutable: bool,
}

pub async fn get_contract(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<ContractDetail>, StatusCode> {
    let contract = state
        .project
        .contracts
        .iter()
        .find(|c| c.name == name)
        .ok_or(StatusCode::NOT_FOUND)?;

    let functions = contract
        .functions
        .iter()
        .map(|f| {
            let key = (contract.name.clone(), f.name.clone());
            let pt = state.path_trees.get(&key);
            FunctionSummary {
                name: f.name.clone(),
                kind: format!("{:?}", f.kind),
                visibility: format!("{:?}", f.visibility),
                mutability: format!("{:?}", f.mutability),
                params: f.params.iter().map(|p| ParamSummary {
                    name: p.name.clone(),
                    type_name: p.type_name.clone(),
                }).collect(),
                path_count: pt.map(|p| p.stats.total_paths).unwrap_or(0),
                happy_paths: pt.map(|p| p.stats.happy_paths).unwrap_or(0),
                revert_paths: pt.map(|p| p.stats.revert_paths).unwrap_or(0),
            }
        })
        .collect();

    let state_vars = contract
        .state_vars
        .iter()
        .map(|sv| StateVarSummary {
            name: sv.name.clone(),
            type_name: sv.type_name.clone(),
            visibility: format!("{:?}", sv.visibility),
            is_constant: sv.is_constant,
            is_immutable: sv.is_immutable,
        })
        .collect();

    Ok(Json(ContractDetail {
        name: contract.name.clone(),
        kind: format!("{:?}", contract.kind),
        inherits: contract.inherits.clone(),
        functions,
        state_vars,
    }))
}

// ============================================================================
// Call graph (Cytoscape-compatible JSON)
// ============================================================================

#[derive(Serialize)]
pub struct CytoscapeGraph {
    pub nodes: Vec<CytoscapeNode>,
    pub edges: Vec<CytoscapeEdge>,
}

#[derive(Serialize)]
pub struct CytoscapeNode {
    pub data: CytoscapeNodeData,
}

#[derive(Serialize)]
pub struct CytoscapeNodeData {
    pub id: String,
    pub label: String,
    pub node_type: String,
    pub contract: String,
    pub is_external: bool,
}

#[derive(Serialize)]
pub struct CytoscapeEdge {
    pub data: CytoscapeEdgeData,
}

#[derive(Serialize)]
pub struct CytoscapeEdgeData {
    pub id: String,
    pub source: String,
    pub target: String,
    pub kind: String,
    pub call_count: usize,
}

pub async fn get_callgraph(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<CytoscapeGraph>, StatusCode> {
    let cg = state
        .call_graphs
        .get(&name)
        .ok_or(StatusCode::NOT_FOUND)?;

    let nodes: Vec<CytoscapeNode> = cg
        .node_indices()
        .map(|idx| {
            let node = &cg[idx];
            CytoscapeNode {
                data: CytoscapeNodeData {
                    id: format!("{}::{}", node.contract, node.function),
                    label: node.function.clone(),
                    node_type: if node.is_external { "external" } else { "internal" }.into(),
                    contract: node.contract.clone(),
                    is_external: node.is_external,
                },
            }
        })
        .collect();

    let edges: Vec<CytoscapeEdge> = cg
        .edge_indices()
        .enumerate()
        .map(|(i, edge_idx)| {
            let (src, dst) = cg.edge_endpoints(edge_idx).unwrap();
            let edge = &cg[edge_idx];
            CytoscapeEdge {
                data: CytoscapeEdgeData {
                    id: format!("e{i}"),
                    source: format!("{}::{}", cg[src].contract, cg[src].function),
                    target: format!("{}::{}", cg[dst].contract, cg[dst].function),
                    kind: format!("{:?}", edge.kind),
                    call_count: edge.call_count,
                },
            }
        })
        .collect();

    Ok(Json(CytoscapeGraph { nodes, edges }))
}

// ============================================================================
// CFG (Cytoscape-compatible JSON)
// ============================================================================

pub async fn get_cfg(
    State(state): State<Arc<AppState>>,
    Path((contract_name, func_name)): Path<(String, String)>,
) -> Result<Json<CytoscapeGraph>, StatusCode> {
    let key = (contract_name, func_name);
    let cfg = state.cfgs.get(&key).ok_or(StatusCode::NOT_FOUND)?;

    let nodes: Vec<CytoscapeNode> = cfg
        .node_indices()
        .map(|idx| {
            let block = &cfg[idx];
            CytoscapeNode {
                data: CytoscapeNodeData {
                    id: format!("b{}", block.id),
                    label: format!("{:?} ({} stmts)", block.kind, block.statements.len()),
                    node_type: format!("{:?}", block.kind),
                    contract: key.0.clone(),
                    is_external: false,
                },
            }
        })
        .collect();

    let edges: Vec<CytoscapeEdge> = cfg
        .edge_indices()
        .enumerate()
        .map(|(i, edge_idx)| {
            let (src, dst) = cfg.edge_endpoints(edge_idx).unwrap();
            let edge = &cfg[edge_idx];
            CytoscapeEdge {
                data: CytoscapeEdgeData {
                    id: format!("e{i}"),
                    source: format!("b{}", cfg[src].id),
                    target: format!("b{}", cfg[dst].id),
                    kind: format!("{:?}", edge),
                    call_count: 0,
                },
            }
        })
        .collect();

    Ok(Json(CytoscapeGraph { nodes, edges }))
}

// ============================================================================
// Path tree
// ============================================================================

pub async fn get_paths(
    State(state): State<Arc<AppState>>,
    Path((contract_name, func_name)): Path<(String, String)>,
) -> Result<Json<PathTree>, StatusCode> {
    let key = (contract_name, func_name);
    state
        .path_trees
        .get(&key)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

// ============================================================================
// Sequence tree
// ============================================================================

#[derive(Deserialize)]
pub struct SequenceQuery {
    pub depth: Option<usize>,
}

pub async fn get_sequences(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Query(_query): Query<SequenceQuery>,
) -> Result<Json<SequenceTree>, StatusCode> {
    // For now return pre-computed tree. In the future, recompute with custom depth.
    state
        .sequence_trees
        .get(&name)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}
