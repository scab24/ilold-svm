use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use ilold_core::exploration::commands::{
    AnalysisData, CommandResult, SessionCommand,
    canvas_patch_from, execute_command, get_function_info, get_sequence_narrative,
    get_session_state, get_step_narrative,
};
use ilold_core::exploration::session::{ExplorationSession, VariableSummary};
use ilold_core::narrative::types::{FunctionNarrative, SequenceNarrative};

use crate::state::AppState;

#[derive(Deserialize)]
pub struct CommandRequest {
    pub contract: Option<String>,
    pub command: SessionCommand,
}

fn build_analysis_data<'a>(
    state: &'a AppState,
    contract_name: &str,
) -> Result<AnalysisData<'a>, (StatusCode, String)> {
    let contract = state.project.contracts.iter()
        .find(|c| c.name == contract_name)
        .ok_or((StatusCode::NOT_FOUND, format!("Contract '{}' not found", contract_name)))?;

    let seq_analysis = state.sequence_analyses.get(contract_name)
        .ok_or((StatusCode::NOT_FOUND, "No analysis for contract".into()))?;

    let classifs = state.classifications.get(contract_name)
        .ok_or((StatusCode::NOT_FOUND, "No classifications for contract".into()))?;

    Ok(AnalysisData {
        project: &state.project,
        contract,
        cfgs: &state.cfgs,
        path_trees: &state.path_trees,
        behaviors: &seq_analysis.functions,
        transitions: &seq_analysis.transitions,
        classifications: classifs,
        all_sequence_analyses: &state.sequence_analyses,
        all_classifications: &state.classifications,
    })
}

fn resolve_contract(state: &AppState, explicit: Option<&str>) -> Result<String, (StatusCode, String)> {
    if let Some(name) = explicit {
        return Ok(name.to_string());
    }

    let session_guard = state.session.read().unwrap();
    if let Some(session) = session_guard.as_ref() {
        return Ok(session.contract.clone());
    }
    drop(session_guard);

    state.project.find_contract(None)
        .map(|c| c.name.clone())
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

fn timestamp_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("{}", d.as_secs()))
        .unwrap_or_default()
}

pub async fn handle_command(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CommandRequest>,
) -> Result<Json<CommandResult>, (StatusCode, String)> {
    let contract_name = resolve_contract(&state, req.contract.as_deref())?;
    let data = build_analysis_data(&state, &contract_name)?;
    let timestamp = timestamp_now();

    let mut session_guard = state.session.write().unwrap();
    let session = session_guard.get_or_insert_with(|| {
        ExplorationSession::new(&contract_name, "ilold")
    });

    let result = execute_command(req.command, session, &data, &timestamp);

    if let Some(patch) = canvas_patch_from(&result) {
        state.session_tx.send(patch).ok();
    }

    Ok(Json(result))
}

pub async fn get_step_detail(
    State(state): State<Arc<AppState>>,
    Path(step_index): Path<usize>,
) -> Result<Json<FunctionNarrative>, (StatusCode, String)> {
    let session_guard = state.session.read().unwrap();
    let session = session_guard.as_ref()
        .ok_or((StatusCode::NOT_FOUND, "No active session".into()))?;

    let data = build_analysis_data(&state, &session.contract)?;

    let narrative = get_step_narrative(session, step_index, &data)
        .map_err(|e| (StatusCode::NOT_FOUND, e))?;

    Ok(Json(narrative))
}

pub async fn get_state_detail(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<VariableSummary>>, (StatusCode, String)> {
    let session_guard = state.session.read().unwrap();
    let session = session_guard.as_ref()
        .ok_or((StatusCode::NOT_FOUND, "No active session".into()))?;

    Ok(Json(get_session_state(session)))
}

pub async fn get_sequence_detail(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SequenceNarrative>, (StatusCode, String)> {
    let session_guard = state.session.read().unwrap();
    let session = session_guard.as_ref()
        .ok_or((StatusCode::NOT_FOUND, "No active session".into()))?;

    let data = build_analysis_data(&state, &session.contract)?;

    let narrative = get_sequence_narrative(session, &data)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(narrative))
}

pub async fn get_function_detail(
    State(state): State<Arc<AppState>>,
    Path((contract_name, func_name)): Path<(String, String)>,
) -> Result<Json<FunctionNarrative>, (StatusCode, String)> {
    let data = build_analysis_data(&state, &contract_name)?;

    let narrative = get_function_info(&func_name, &data)
        .map_err(|e| (StatusCode::NOT_FOUND, e))?;

    Ok(Json(narrative))
}
