use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use ilold_core::model::common::SourceSpan;
use ilold_solana_core::model::ProgramDef;
use ilold_solana_core::overlay::RuntimeOverlay;
use ilold_solana_core::view::ProgramView;
use serde::{Deserialize, Serialize};
use solana_keypair::Signer;
use syn::spanned::Spanned;

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

pub async fn get_program_view(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<ProgramView>, (StatusCode, String)> {
    let program = find_solana_program(&state, &name)?;
    Ok(Json(program.compute_view()))
}

/// Resolve the per-scenario `pubkey -> user name` map. Lets the canvas show
/// "alice" instead of "Bxk7…" when a runtime field matches a known user.
/// Lives in its own endpoint (not on `RuntimeOverlay`) because users mutate
/// independently of the overlay; see design.md §"authority_resolutions".
pub async fn get_user_labels(
    State(state): State<Arc<AppState>>,
    Path(scenario): Path<String>,
) -> Result<Json<HashMap<String, String>>, (StatusCode, String)> {
    let solana = state
        .solana()
        .ok_or((StatusCode::BAD_REQUEST, "endpoint is Solana-only".into()))?;
    let users_lock = solana.users.read().unwrap();
    let scn_users = users_lock.get(&scenario).ok_or((
        StatusCode::NOT_FOUND,
        format!("scenario '{scenario}' has no user registry"),
    ))?;
    let map: HashMap<String, String> = scn_users
        .iter()
        .map(|(name, kp)| (kp.pubkey().to_string(), name.clone()))
        .collect();
    Ok(Json(map))
}

#[derive(Deserialize, Default)]
pub struct OverlayQuery {
    pub scenario: Option<String>,
}

pub async fn get_program_overlay(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Query(params): Query<OverlayQuery>,
) -> Result<Json<RuntimeOverlay>, (StatusCode, String)> {
    let program = find_solana_program(&state, &name)?;
    let scenarios = state.scenarios.read().unwrap();
    let scenario_name = params
        .scenario
        .clone()
        .unwrap_or_else(|| scenarios.active().to_string());
    let session = scenarios.get(&scenario_name).ok_or((
        StatusCode::NOT_FOUND,
        format!("scenario '{scenario_name}' not found"),
    ))?;
    let mut overlay = RuntimeOverlay::from_session(session);
    if overlay.program.is_empty() {
        overlay.program = program.name.clone();
    }
    overlay.scenario = scenario_name;
    Ok(Json(overlay))
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

// ============================================================================
// Instruction source — Anchor handler body for the canvas "View source" panel
// and the IDE deep link. Mirrors `get_function_source` for Solidity.
// ============================================================================

/// Shape mirrors `FunctionSourceResponse` so the frontend can reuse the
/// `FunctionSourcePanel` component without a parallel type.
#[derive(Serialize)]
pub struct InstructionSourceResponse {
    pub file_path: String,
    pub source: String,
    pub span: SourceSpan,
}

pub async fn get_instruction_source(
    State(state): State<Arc<AppState>>,
    Path((name, ix)): Path<(String, String)>,
) -> Result<Json<InstructionSourceResponse>, (StatusCode, String)> {
    let program = find_solana_program(&state, &name)?;
    // Reject early: 404 if the IDL has no such instruction. Avoids reading
    // the file just to return the same error after parsing.
    if !program.instructions.iter().any(|i| i.name == ix) {
        return Err((
            StatusCode::NOT_FOUND,
            format!("instruction '{ix}' not found in program '{name}'"),
        ));
    }

    let file_path = state
        .project_root
        .join("programs")
        .join(&program.name)
        .join("src")
        .join("lib.rs");
    let content = std::fs::read_to_string(&file_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("read {}: {e}", file_path.display()),
        )
    })?;

    let span = extract_handler_span(&content, &ix).ok_or((
        StatusCode::NOT_FOUND,
        format!("handler '{ix}' not found in {}", file_path.display()),
    ))?;

    let source = slice_lines(&content, span.start_line as usize, span.end_line as usize);

    Ok(Json(InstructionSourceResponse {
        file_path: file_path.to_string_lossy().into_owned(),
        source,
        span,
    }))
}

/// Parse `lib.rs` with syn and locate the `pub fn <ix>` inside the
/// `#[program] pub mod ...` module. Falls back to top-level fns and to a
/// recursive walk so handlers nested in unusual layouts still resolve.
pub(crate) fn extract_handler_span(source: &str, ix: &str) -> Option<SourceSpan> {
    let file = syn::parse_file(source).ok()?;
    find_fn_in_items(&file.items, ix)
}

fn find_fn_in_items(items: &[syn::Item], ix: &str) -> Option<SourceSpan> {
    // Prefer the `#[program]`-attributed module — that is where Anchor places
    // the IDL-visible handlers, so we avoid colliding with helper fns named
    // the same elsewhere in the file.
    for item in items {
        if let syn::Item::Mod(m) = item {
            if has_program_attr(&m.attrs) {
                if let Some((_, inner)) = &m.content {
                    if let Some(span) = find_fn_in_items(inner, ix) {
                        return Some(span);
                    }
                }
            }
        }
    }
    for item in items {
        match item {
            syn::Item::Fn(f) if f.sig.ident == ix => return Some(span_of(f)),
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    if let Some(span) = find_fn_in_items(inner, ix) {
                        return Some(span);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn has_program_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| a.path().is_ident("program"))
}

fn span_of(item: &syn::ItemFn) -> SourceSpan {
    let span = item.span();
    let start = span.start();
    let end = span.end();
    SourceSpan {
        file_index: 0,
        start_line: start.line as u32,
        start_col: start.column as u32,
        end_line: end.line as u32,
        end_col: end.column as u32,
    }
}

fn slice_lines(src: &str, start_1based: usize, end_1based: usize) -> String {
    if start_1based == 0 || end_1based < start_1based {
        return String::new();
    }
    src.lines()
        .skip(start_1based - 1)
        .take(end_1based - start_1based + 1)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAKING_LIB: &str = include_str!(
        "../../../../tests/fixtures/solana/staking/programs/staking/src/lib.rs"
    );

    #[test]
    fn extract_handler_span_finds_stake() {
        let span = extract_handler_span(STAKING_LIB, "stake").expect("stake handler");
        // `pub fn stake(...)` is on line 19 in the fixture (1-based).
        assert_eq!(span.start_line, 19, "start line off: {span:?}");
        assert!(
            span.end_line > span.start_line,
            "stake body must span >1 lines: {span:?}",
        );
        let body = slice_lines(STAKING_LIB, span.start_line as usize, span.end_line as usize);
        assert!(body.contains("pub fn stake"), "body missing signature:\n{body}");
        assert!(
            body.contains("StakingError::ZeroAmount"),
            "body missing require!:\n{body}",
        );
    }

    #[test]
    fn extract_handler_span_missing_returns_none() {
        assert!(extract_handler_span(STAKING_LIB, "does_not_exist").is_none());
    }
}
