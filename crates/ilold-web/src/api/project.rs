use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use ilold_solana_core::model::ProgramDef;
use ilold_solana_core::view::ProgramView;
use serde::Serialize;

use crate::state::{require_solidity_msg, AppState, Backend};

fn find_solana_program(
    state: &Arc<AppState>,
    name: &str,
) -> Result<ProgramDef, (StatusCode, String)> {
    let solana = state
        .solana()
        .ok_or((StatusCode::BAD_REQUEST, "endpoint is Solana-only".into()))?;
    solana
        .project
        .find_program(name)
        .cloned()
        .ok_or((StatusCode::NOT_FOUND, format!("program '{name}' not found")))
}

#[derive(Serialize)]
pub struct ProjectSummary {
    pub files: usize,
    pub contracts: Vec<ContractSummary>,
}

#[derive(Serialize)]
pub struct ContractSummary {
    pub name: String,
    pub kind: String,
    pub functions: usize,
    pub state_vars: usize,
    pub inherits: Vec<String>,
}

pub async fn get_project(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ProjectSummary>, (StatusCode, String)> {
    let s = require_solidity_msg(&state)?;
    let contracts = s
        .project
        .contracts
        .iter()
        .map(|c| ContractSummary {
            name: c.name.clone(),
            kind: format!("{:?}", c.kind),
            functions: c.functions.len(),
            state_vars: c.state_vars.len(),
            inherits: c.inherits.clone(),
        })
        .collect();

    Ok(Json(ProjectSummary {
        files: s.project.source_files.len(),
        contracts,
    }))
}

// ============================================================================
// Project Map — full contract details with cross-contract relationships
// ============================================================================

#[derive(Serialize)]
pub struct ProjectMap {
    pub kind: &'static str,
    pub contracts: Vec<MapContract>,
    pub programs: Vec<MapProgram>,
    pub relationships: Vec<MapRelationship>,
}

#[derive(Serialize)]
pub struct MapProgram {
    pub name: String,
    pub program_id: String,
    pub instructions: Vec<MapInstruction>,
    pub account_types: Vec<MapAccountType>,
}

#[derive(Serialize)]
pub struct MapInstruction {
    pub name: String,
    pub args_count: usize,
    pub accounts_count: usize,
    pub has_pdas: bool,
}

#[derive(Serialize)]
pub struct MapAccountType {
    pub name: String,
}

#[derive(Serialize)]
pub struct MapContract {
    pub name: String,
    pub kind: String,
    pub inherits: Vec<String>,
    pub functions: Vec<MapFunction>,
    pub state_vars: Vec<MapStateVar>,
}

#[derive(Serialize)]
pub struct MapFunction {
    pub name: String,
    pub visibility: String,
    pub mutability: String,
    pub path_count: usize,
    pub happy_paths: usize,
    pub revert_paths: usize,
    pub has_external_calls: bool,
}

#[derive(Serialize)]
pub struct MapStateVar {
    pub name: String,
    pub type_name: String,
    pub is_constant: bool,
}

#[derive(Serialize)]
pub struct MapRelationship {
    pub from_contract: String,
    pub from_function: String,
    pub to_contract: String,
    pub to_function: String,
    pub kind: String,
}

pub async fn get_program_detail(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<ProgramDef>, (StatusCode, String)> {
    find_solana_program(&state, &name).map(Json)
}

pub async fn get_program_view(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<ProgramView>, (StatusCode, String)> {
    let program = find_solana_program(&state, &name)?;
    Ok(Json(program.compute_view()))
}

pub async fn get_project_map(
    State(state): State<Arc<AppState>>,
) -> Json<ProjectMap> {
    match &state.backend {
        Backend::Solidity(_) => Json(build_solidity_map(&state)),
        Backend::Solana(s) => Json(build_solana_map(s)),
    }
}

fn build_solana_map(s: &crate::state::SolanaState) -> ProjectMap {
    let programs: Vec<MapProgram> = s
        .project
        .programs
        .iter()
        .map(|p| MapProgram {
            name: p.name.clone(),
            program_id: p.program_id.to_string(),
            instructions: p
                .instructions
                .iter()
                .map(|ix| MapInstruction {
                    name: ix.name.clone(),
                    args_count: ix.args.len(),
                    accounts_count: ix.accounts.len(),
                    has_pdas: ix.accounts.iter().any(|a| a.pda.is_some()),
                })
                .collect(),
            account_types: p
                .account_types
                .iter()
                .map(|a| MapAccountType { name: a.name.clone() })
                .collect(),
        })
        .collect();

    ProjectMap {
        kind: "solana",
        contracts: Vec::new(),
        programs,
        relationships: Vec::new(),
    }
}

fn build_solidity_map(state: &Arc<AppState>) -> ProjectMap {
    let s = state.unwrap_solidity();
    let mut contracts = Vec::new();
    let mut relationships = Vec::new();

    for contract in &s.project.contracts {
        let functions: Vec<MapFunction> = contract
            .functions
            .iter()
            .filter(|f| !f.name.is_empty())
            .map(|f| {
                let key = (contract.name.clone(), f.name.clone());
                let pt = s.path_trees.get(&key);
                let has_ext = pt
                    .map(|p| p.paths.iter().any(|path| !path.annotations.external_calls.is_empty()))
                    .unwrap_or(false);
                MapFunction {
                    name: f.name.clone(),
                    visibility: format!("{:?}", f.visibility),
                    mutability: format!("{:?}", f.mutability),
                    path_count: pt.map(|p| p.stats.total_paths).unwrap_or(0),
                    happy_paths: pt.map(|p| p.stats.happy_paths).unwrap_or(0),
                    revert_paths: pt.map(|p| p.stats.revert_paths).unwrap_or(0),
                    has_external_calls: has_ext,
                }
            })
            .collect();

        let state_vars: Vec<MapStateVar> = contract
            .state_vars
            .iter()
            .map(|sv| MapStateVar {
                name: sv.name.clone(),
                type_name: sv.type_name.clone(),
                is_constant: sv.is_constant,
            })
            .collect();

        contracts.push(MapContract {
            name: contract.name.clone(),
            kind: format!("{:?}", contract.kind),
            inherits: contract.inherits.clone(),
            functions,
            state_vars,
        });

        if let Some(cg) = s.call_graphs.get(&contract.name) {
            for edge_idx in cg.edge_indices() {
                let (src, dst) = cg.edge_endpoints(edge_idx).unwrap();
                let src_node = &cg[src];
                let dst_node = &cg[dst];
                let edge = &cg[edge_idx];

                if dst_node.is_external || src_node.contract != dst_node.contract {
                    relationships.push(MapRelationship {
                        from_contract: src_node.contract.clone(),
                        from_function: src_node.function.clone(),
                        to_contract: dst_node.contract.clone(),
                        to_function: dst_node.function.clone(),
                        kind: format!("{:?}", edge.kind),
                    });
                }
            }
        }
    }

    ProjectMap {
        kind: "solidity",
        contracts,
        programs: Vec::new(),
        relationships,
    }
}
