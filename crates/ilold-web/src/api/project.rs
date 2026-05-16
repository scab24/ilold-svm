use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use ilold_solana_core::model::ProgramDef;
use ilold_solana_core::overlay::RuntimeOverlay;
use ilold_solana_core::view::ProgramView;
use serde::{Deserialize, Serialize};
use solana_keypair::Signer;
use syn::spanned::Spanned;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct SourceSpan {
    pub file_index: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

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
    pub kind: &'static str,
    pub programs: Vec<ProgramSummary>,
}

#[derive(Serialize)]
pub struct ProgramSummary {
    pub name: String,
    pub program_id: String,
    pub instructions: usize,
    pub account_types: usize,
}

pub async fn get_project(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ProjectSummary>, (StatusCode, String)> {
    let solana = state
        .solana()
        .ok_or((StatusCode::BAD_REQUEST, "endpoint is Solana-only".into()))?;
    let programs = solana
        .project
        .programs
        .iter()
        .map(|p| ProgramSummary {
            name: p.name.clone(),
            program_id: p.program_id.to_string(),
            instructions: p.instructions.len(),
            account_types: p.account_types.len(),
        })
        .collect();
    Ok(Json(ProjectSummary {
        kind: "solana",
        programs,
    }))
}

#[derive(Serialize)]
pub struct ProjectMap {
    pub kind: &'static str,
    pub programs: Vec<MapProgram>,
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

pub async fn get_program_view(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<ProgramView>, (StatusCode, String)> {
    let program = find_solana_program(&state, &name)?;
    Ok(Json(program.compute_view()))
}

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
    let solana = match state.solana() {
        Some(s) => s,
        None => {
            return Json(ProjectMap {
                kind: "solana",
                programs: Vec::new(),
            });
        }
    };
    Json(build_solana_map(solana))
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
        programs,
    }
}

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
    let file_path = std::fs::canonicalize(&file_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("canonicalize {}: {e}", file_path.display()),
        )
    })?;
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

pub(crate) fn extract_handler_span(source: &str, ix: &str) -> Option<SourceSpan> {
    let file = syn::parse_file(source).ok()?;
    find_fn_in_items(&file.items, ix)
}

fn find_fn_in_items(items: &[syn::Item], ix: &str) -> Option<SourceSpan> {
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
