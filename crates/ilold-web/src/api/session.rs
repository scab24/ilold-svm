use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use ilold_core::classify::entry_points::AccessLevel;
use ilold_core::exploration::commands::{
    AnalysisData, CanvasPatch, CommandResult, ScenarioAction, ScenarioInfo, SessionCommand,
    canvas_patch_from, execute_command, get_flow_tree, get_function_info, get_sequence_narrative,
    get_session_state, get_step_narrative, validate_scenario_name,
};
use ilold_core::exploration::session::{ExplorationSession, ForkOrigin, VariableSummary};
use ilold_core::journal::types::JournalEntry;
use ilold_core::exploration::timeline::{build_variable_timeline, VariableTimeline};
use ilold_core::narrative::trace::FlowTree;
use ilold_core::narrative::types::{FunctionNarrative, SequenceNarrative};
use ilold_core::slicing::{build_slice_result, SliceDirection, SliceResult};

use crate::state::{AppState, ScenarioStore};

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

    let scenarios_guard = state.scenarios.read().unwrap();
    let session = scenarios_guard.active_session();
    if !session.contract.is_empty() {
        return Ok(session.contract.clone());
    }
    drop(scenarios_guard);

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

/// Validate a scenario name and check it is not already taken in the store.
/// Shared guard for both `ScenarioAction::New` and `ScenarioAction::Fork`.
/// Returns `Err(CommandResult::Error)` ready to be propagated.
fn reserve_name(store: &ScenarioStore, name: &str) -> Result<(), CommandResult> {
    if let Err(e) = validate_scenario_name(name) {
        return Err(CommandResult::Error { message: e });
    }
    if store.contains(name) {
        return Err(CommandResult::Error {
            message: format!("Scenario '{name}' already exists"),
        });
    }
    Ok(())
}

/// Execute scenario lifecycle commands against the `ScenarioStore`. Lives
/// in the web crate because it needs `&mut ScenarioStore` (crate boundary);
/// `commands.rs` only defines the data types.
fn execute_scenario(
    store: &mut ScenarioStore,
    action: ScenarioAction,
    timestamp: &str,
    contract: &str,
) -> CommandResult {
    match action {
        ScenarioAction::New { name } => {
            if let Err(err) = reserve_name(store, &name) {
                return err;
            }
            let session = ExplorationSession::new(contract, "ilold");
            store.insert(name.clone(), session);
            CommandResult::ScenarioCreated { name }
        }
        ScenarioAction::List => {
            let active = store.active().to_string();
            let items: Vec<ScenarioInfo> = store
                .names()
                .iter()
                .map(|n| ScenarioInfo {
                    name: n.clone(),
                    active: n == &active,
                    step_count: store.get(n).map(|s| s.steps.len()).unwrap_or(0),
                })
                .collect();
            CommandResult::ScenarioList { items }
        }
        ScenarioAction::Switch { name } => {
            let from = store.active().to_string();
            if name == from {
                // idempotent no-op per spec S3.4; caller is responsible for
                // suppressing WS broadcast when from == to.
                return CommandResult::ScenarioSwitched { from, to: name };
            }
            match store.set_active(name.clone()) {
                Ok(()) => CommandResult::ScenarioSwitched { from, to: name },
                Err(e) => CommandResult::Error { message: e },
            }
        }
        ScenarioAction::Fork { name, at_step } => fork_scenario(store, name, at_step, timestamp),
        ScenarioAction::Delete { name } => {
            if name == store.active() {
                return CommandResult::Error {
                    message: "Cannot delete active scenario — switch first.".into(),
                };
            }
            if store.len() == 1 {
                return CommandResult::Error {
                    message: "Cannot delete the only remaining scenario.".into(),
                };
            }
            if !store.contains(&name) {
                return CommandResult::Error {
                    message: format!("Scenario '{name}' does not exist"),
                };
            }
            store.remove(&name);
            CommandResult::ScenarioDeleted { name }
        }
    }
}

fn fork_scenario(
    store: &mut ScenarioStore,
    new_name: String,
    at_step: Option<usize>,
    timestamp: &str,
) -> CommandResult {
    if let Err(err) = reserve_name(store, &new_name) {
        return err;
    }
    let from = store.active().to_string();
    let mut cloned = store.active_session().clone();

    // Resolve effective step count. None (legacy) → keep all steps.
    // Some(N) → truncate to first N; error if N > current length.
    // Mutations live inside each ExplorationStep, so truncating `steps`
    // drops their owning step's mutations as well.
    let len = cloned.steps.len();
    let effective = match at_step {
        None => len,
        Some(n) if n > len => {
            let noun = if len == 1 { "step" } else { "steps" };
            return CommandResult::Error {
                message: format!(
                    "Cannot fork at step {n}: only {len} {noun} in active scenario"
                ),
            };
        }
        Some(n) => {
            cloned.steps.truncate(n);
            n
        }
    };

    // Record the fork origin on the cloned session itself. The frontend
    // reads this (via /api/scenarios/all) to render the scenario as a
    // branch from its source instead of a parallel timeline.
    cloned.forked_from = Some(ForkOrigin {
        scenario: from.clone(),
        at_step: effective,
    });
    // The `BranchCreated` variant's field names (`from_function`/`branch_function`)
    // are reused here as scenario names per design §2.4 — intentionally not
    // renamed to preserve save-file compatibility.
    cloned.journal.record(JournalEntry::BranchCreated {
        from_function: from.clone(),
        branch_function: new_name.clone(),
        timestamp: timestamp.to_string(),
    });
    store.insert(new_name.clone(), cloned);
    CommandResult::ScenarioForked {
        from,
        to: new_name,
        at_step: effective,
    }
}

pub async fn handle_command(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CommandRequest>,
) -> Result<Json<CommandResult>, (StatusCode, String)> {
    let contract_name = resolve_contract(&state, req.contract.as_deref())?;
    let data = build_analysis_data(&state, &contract_name)?;
    let timestamp = timestamp_now();

    let mut scenarios_guard = state.scenarios.write().unwrap();
    // SaveSession/LoadSession bypass the contract-switch reset: Save doesn't
    // care about the request's contract field, and Load carries its own
    // contract inside the JSON. Without the bypass, loading a file from a
    // different contract would clear the store before the load runs.
    let is_persistence = matches!(
        req.command,
        SessionCommand::SaveSession | SessionCommand::LoadSession { .. }
    );
    // Contract switch: only reset if the active session has actual steps.
    // An empty session with a mismatched contract means the auto-seed picked
    // a default contract that didn't match the first real request — just
    // swap it transparently (no ClearAll, nothing was ever on the canvas).
    let needs_reset = !is_persistence
        && scenarios_guard.active_session().contract != contract_name;
    if needs_reset {
        let had_steps = !scenarios_guard.active_session().steps.is_empty();
        let active_before_reset = scenarios_guard.active().to_string();
        *scenarios_guard = ScenarioStore::new_for_contract(&contract_name);
        if had_steps {
            state.session_tx.send(CanvasPatch::ClearAll {
                scenario: active_before_reset,
            }).ok();
        }
    }
    // Scenario / Save / Load commands operate on the store itself; all
    // other commands are delegated to the active session. Dispatch happens
    // here (before `active_session_mut`) to avoid partial-move errors on
    // `req.command`.
    match req.command {
        SessionCommand::Scenario { sub } => {
            // Capture the active scenario BEFORE executing the scenario
            // command. Switch/Delete mutate `active` or remove scenarios; the
            // lifecycle patch embeds the pre-call name for consistent
            // routing on the frontend.
            let active_before = scenarios_guard.active().to_string();
            let result = execute_scenario(&mut scenarios_guard, sub, &timestamp, &contract_name);
            if let Some(patch) = canvas_patch_from(&result, &active_before) {
                state.session_tx.send(patch).ok();
            }
            Ok(Json(result))
        }
        SessionCommand::SaveSession => {
            let active_before = scenarios_guard.active().to_string();
            let result = match scenarios_guard.save_to_json() {
                Ok(json) => CommandResult::SessionSaved { json },
                Err(message) => CommandResult::Error { message },
            };
            if let Some(patch) = canvas_patch_from(&result, &active_before) {
                state.session_tx.send(patch).ok();
            }
            Ok(Json(result))
        }
        SessionCommand::LoadSession { json } => {
            let result = match ScenarioStore::load_from_json(&json) {
                Ok(loaded) => {
                    let contract = loaded.contract.clone();
                    let step_names: Vec<String> = loaded
                        .active_session()
                        .steps
                        .iter()
                        .map(|s| s.function.clone())
                        .collect();
                    *scenarios_guard = loaded;
                    CommandResult::SessionLoaded { contract, steps: step_names }
                }
                Err(message) => CommandResult::Error { message },
            };
            // Use the post-load active so the WS Reloaded event names the
            // scenario the frontend should switch to.
            let active_after = scenarios_guard.active().to_string();
            if let Some(patch) = canvas_patch_from(&result, &active_after) {
                state.session_tx.send(patch).ok();
            }
            Ok(Json(result))
        }
        other => {
            // Capture the active scenario name BEFORE delegating — the
            // session mutation below doesn't change `active`, but grabbing
            // it here keeps the pattern symmetric with the scenario branch
            // and avoids a second borrow after `active_session_mut`.
            let active_name = scenarios_guard.active().to_string();
            let session = scenarios_guard.active_session_mut();
            let result = execute_command(other, session, &data, &timestamp);
            if let Some(patch) = canvas_patch_from(&result, &active_name) {
                state.session_tx.send(patch).ok();
            }
            Ok(Json(result))
        }
    }
}

pub async fn get_step_detail(
    State(state): State<Arc<AppState>>,
    Path(step_index): Path<usize>,
) -> Result<Json<FunctionNarrative>, (StatusCode, String)> {
    let scenarios_guard = state.scenarios.read().unwrap();
    let session = scenarios_guard.active_session();

    let data = build_analysis_data(&state, &session.contract)?;

    let narrative = get_step_narrative(session, step_index, &data)
        .map_err(|e| (StatusCode::NOT_FOUND, e))?;

    Ok(Json(narrative))
}

/// Return the persisted FlowTree of a session step. The tree is read
/// directly from `step.flow_tree` — no recomputation against the source
/// — so the result reflects what the auditor saw when `c <func>` ran.
pub async fn get_session_step_trace(
    State(state): State<Arc<AppState>>,
    Path(step_index): Path<usize>,
) -> Result<Json<FlowTree>, (StatusCode, String)> {
    let scenarios_guard = state.scenarios.read().unwrap();
    let session = scenarios_guard.active_session();

    let step = session.steps.get(step_index)
        .ok_or((StatusCode::NOT_FOUND, format!("step {} not found", step_index)))?;

    let tree = step.flow_tree.clone()
        .ok_or((
            StatusCode::NOT_FOUND,
            format!(
                "step {} has no persisted trace (loaded from a pre-Phase-2a session); \
                 use 'tr <func>' to rebuild from source",
                step_index
            ),
        ))?;

    Ok(Json(tree))
}

/// Cross-step variable history with path conditions for each write.
pub async fn get_variable_timeline_handler(
    State(state): State<Arc<AppState>>,
    Path(variable): Path<String>,
) -> Result<Json<VariableTimeline>, (StatusCode, String)> {
    let scenarios_guard = state.scenarios.read().unwrap();
    let session = scenarios_guard.active_session();

    let timeline = build_variable_timeline(session, &variable);
    Ok(Json(timeline))
}

pub async fn get_state_detail(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<VariableSummary>>, (StatusCode, String)> {
    let scenarios_guard = state.scenarios.read().unwrap();
    let session = scenarios_guard.active_session();

    Ok(Json(get_session_state(session)))
}

pub async fn get_sequence_detail(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SequenceNarrative>, (StatusCode, String)> {
    let scenarios_guard = state.scenarios.read().unwrap();
    let session = scenarios_guard.active_session();

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

#[derive(Deserialize)]
pub struct TraceQuery {
    #[serde(default)]
    pub depth: Option<usize>,
    #[serde(default)]
    pub reverts: Option<bool>,
    /// Comma-separated step_ids to force-inline beyond `depth`. Example:
    /// `?expand=17,24` will inline both calls regardless of max_depth.
    #[serde(default)]
    pub expand: Option<String>,
}

pub async fn get_flow_trace(
    State(state): State<Arc<AppState>>,
    Path((contract_name, func_name)): Path<(String, String)>,
    Query(params): Query<TraceQuery>,
) -> Result<Json<FlowTree>, (StatusCode, String)> {
    let data = build_analysis_data(&state, &contract_name)?;

    let max_depth = params.depth.unwrap_or(2);
    let include_reverts = params.reverts.unwrap_or(false);
    let expand_set = parse_expand_set(params.expand.as_deref())
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let tree = get_flow_tree(&func_name, &data, max_depth, include_reverts, expand_set)
        .map_err(|e| (StatusCode::NOT_FOUND, e))?;

    Ok(Json(tree))
}

#[derive(Deserialize)]
pub struct SliceQuery {
    /// `backward`, `forward`, or `both`. Defaults to `both` when absent.
    /// Short forms `b`/`f` and synonyms `back`/`fwd`/`all` are accepted.
    #[serde(default)]
    pub direction: Option<String>,
}

/// Dataflow slice for `variable` inside `function` of the session's
/// current contract. The function is resolved from the active session so
/// the auditor doesn't have to re-type the contract name; if no session
/// exists the endpoint returns 404.
pub async fn get_function_slice(
    State(state): State<Arc<AppState>>,
    Path((func_name, variable)): Path<(String, String)>,
    Query(params): Query<SliceQuery>,
) -> Result<Json<SliceResult>, (StatusCode, String)> {
    let contract_name = {
        let guard = state.scenarios.read().unwrap();
        guard.active_session().contract.clone()
    };

    let contract = state.project.contracts.iter()
        .find(|c| c.name == contract_name)
        .ok_or((
            StatusCode::NOT_FOUND,
            format!("Contract '{}' not found", contract_name),
        ))?;

    let function = contract.functions.iter()
        .find(|f| f.name == func_name)
        .ok_or((
            StatusCode::NOT_FOUND,
            format!("Function '{}' not found in {}", func_name, contract_name),
        ))?;

    let direction = parse_slice_direction(params.direction.as_deref())
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(build_slice_result(
        &state.project,
        contract,
        function,
        &variable,
        direction,
    )))
}

fn parse_slice_direction(raw: Option<&str>) -> Result<SliceDirection, String> {
    let Some(value) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(SliceDirection::Both);
    };
    match value.to_ascii_lowercase().as_str() {
        "backward" | "back" | "b" => Ok(SliceDirection::Backward),
        "forward" | "fwd" | "f" => Ok(SliceDirection::Forward),
        "both" | "all" => Ok(SliceDirection::Both),
        other => Err(format!(
            "invalid direction {:?}, expected backward|forward|both",
            other
        )),
    }
}

/// Lightweight per-step view for the `/api/scenarios/all` snapshot. The
/// access level is resolved against `AppState.classifications` (same lookup
/// pattern used by `execute_who` in the core crate) so the frontend can
/// colour the node without re-classifying.
#[derive(Serialize)]
pub struct SessionStepView {
    pub function: String,
    pub access: AccessLevel,
    pub step_index: usize,
}

/// Per-scenario snapshot in `AllScenariosResponse`. `forked_from` is `None`
/// for `main` and for any scenario loaded from a pre-fork-origin save file.
#[derive(Serialize)]
pub struct ScenarioSnapshot {
    pub name: String,
    pub steps: Vec<SessionStepView>,
    pub forked_from: Option<ForkOrigin>,
}

#[derive(Serialize)]
pub struct AllScenariosResponse {
    pub active: String,
    /// Ordered by creation order (main first, then insertion order). Not a
    /// HashMap — the frontend composes scenarios into a visual tree and needs
    /// stable iteration so "main" always anchors the canvas.
    pub scenarios: Vec<ScenarioSnapshot>,
}

/// Return the list of scenarios — mirrors the `scenario list` CLI command.
pub async fn get_scenarios(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<ScenarioInfo>> {
    let guard = state.scenarios.read().unwrap();
    let active = guard.active().to_string();
    let items = guard
        .names()
        .iter()
        .map(|name| ScenarioInfo {
            name: name.clone(),
            active: name == &active,
            step_count: guard.get(name).map(|s| s.steps.len()).unwrap_or(0),
        })
        .collect();
    Json(items)
}

/// Bulk snapshot of every scenario's step list. Used by the frontend canvas
/// to paint all scenarios at once (e.g. on reconnect / initial load).
pub async fn get_all_scenarios(
    State(state): State<Arc<AppState>>,
) -> Json<AllScenariosResponse> {
    let guard = state.scenarios.read().unwrap();
    let active = guard.active().to_string();
    let mut scenarios: Vec<ScenarioSnapshot> = Vec::with_capacity(guard.len());
    for name in guard.names() {
        let Some(session) = guard.get(name) else { continue };
        let classifs = state.classifications.get(&session.contract);
        let steps = session
            .steps
            .iter()
            .enumerate()
            .map(|(idx, step)| {
                // Same lookup pattern as `execute_who`'s `access_for`
                // (`commands.rs:515-519`): fall back to `Internal` when the
                // classification is missing so the response shape is stable.
                let access = classifs
                    .and_then(|c| c.iter().find(|(n, _)| n == &step.function))
                    .map(|(_, a)| a.clone())
                    .unwrap_or(AccessLevel::Internal);
                SessionStepView {
                    function: step.function.clone(),
                    access,
                    step_index: idx,
                }
            })
            .collect();
        scenarios.push(ScenarioSnapshot {
            name: name.clone(),
            steps,
            forked_from: session.forked_from.clone(),
        });
    }
    Json(AllScenariosResponse { active, scenarios })
}

/// Parse a comma-separated `expand` query value into a set of step_ids.
/// Empty input → empty set. Whitespace around values is tolerated.
/// Returns `Err` with a descriptive message if any value is not a usize.
fn parse_expand_set(raw: Option<&str>) -> Result<std::collections::HashSet<usize>, String> {
    let mut set = std::collections::HashSet::new();
    let raw = match raw {
        Some(s) if !s.trim().is_empty() => s,
        _ => return Ok(set),
    };
    for part in raw.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let id: usize = trimmed.parse()
            .map_err(|_| format!("invalid step_id in expand: {:?}", trimmed))?;
        set.insert(id);
    }
    Ok(set)
}
