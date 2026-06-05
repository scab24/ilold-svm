use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use ilold_session_core::exploration::scenario::{
    ScenarioAction as SharedScenarioAction, ScenarioInfo,
};
use ilold_session_core::exploration::session::{ExplorationSession, ForkOrigin};
use ilold_session_core::journal::types::JournalEntry;
use ilold_solana_core::exploration::{
    canvas_patches_from_solana, execute_airdrop, execute_back, execute_call, execute_clear,
    execute_coupling, execute_coverage, execute_export, execute_findings_list, execute_finding,
    execute_funcs, execute_info, execute_inspect, execute_note, execute_pda, execute_session,
    execute_state, execute_status, execute_step, execute_time_warp, execute_timeline,
    execute_users, execute_users_new, execute_vars, execute_who, SolanaCommand,
    SolanaCommandResult,
};
use solana_keypair::Keypair;

use crate::state::{AppState, ScenarioStore};

#[derive(Deserialize)]
pub struct CommandRequest {
    pub contract: Option<String>,
    pub command: Value,
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
) -> Result<Json<Value>, (StatusCode, String)> {
    handle_solana_command(state, req).await
}

async fn handle_solana_command(
    state: Arc<AppState>,
    req: CommandRequest,
) -> Result<Json<Value>, (StatusCode, String)> {
    let solana = state
        .solana()
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "solana state missing".into()))?;

    let command: SolanaCommand = serde_json::from_value(req.command)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid Solana command: {e}")))?;

    let timestamp = timestamp_now();

    let program = match req.contract.as_deref() {
        Some(name) => solana
            .project
            .find_program(name)
            .ok_or((
                StatusCode::NOT_FOUND,
                format!("program '{name}' not found"),
            ))?
            .clone(),
        None => solana
            .project
            .programs
            .first()
            .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "no programs available".into()))?
            .clone(),
    };

    if let SolanaCommand::Scenario { sub } = command {
        let mut scenarios = state.scenarios.write().unwrap();
        let active_before = scenarios.active().to_string();
        let result = solana_scenario_action(
            &mut scenarios,
            solana,
            sub,
            &active_before,
            &timestamp,
            &program.name,
        );
        for patch in canvas_patches_from_solana(&result, &active_before, &[]) {
            state.session_tx.send(patch).ok();
        }
        return Ok(Json(serde_json::to_value(result).unwrap_or(Value::Null)));
    }

    if let SolanaCommand::SaveSession { with_keypairs } = &command {
        let scenarios = state.scenarios.read().unwrap();
        let users_snapshot = if *with_keypairs {
            let users_lock = solana.users.read().unwrap();
            let cloned: std::collections::HashMap<
                String,
                std::collections::HashMap<String, Keypair>,
            > = users_lock
                .iter()
                .map(|(scn, map)| {
                    let inner: std::collections::HashMap<String, Keypair> = map
                        .iter()
                        .map(|(name, kp)| (name.clone(), kp.insecure_clone()))
                        .collect();
                    (scn.clone(), inner)
                })
                .collect();
            Some(cloned)
        } else {
            None
        };
        let opts = crate::state::SaveOpts {
            keypairs: users_snapshot.as_ref(),
        };
        let result = match scenarios.save_to_json(opts) {
            Ok(json) => SolanaCommandResult::SessionSaved { json },
            Err(message) => SolanaCommandResult::Error { message },
        };
        return Ok(Json(serde_json::to_value(result).unwrap_or(Value::Null)));
    }
    if let SolanaCommand::LoadSession { json } = command {
        let mut scenarios = state.scenarios.write().unwrap();
        let mut vms = solana.vms.write().unwrap();
        let mut users_lock = solana.users.write().unwrap();
        let mut snapshots = solana.step_snapshots.write().unwrap();
        let result = match ScenarioStore::load_from_json(&json) {
            Ok((loaded, kp_bundle)) => {
                let prog = loaded.contract.clone();
                let step_names: Vec<String> = loaded
                    .active_session()
                    .steps
                    .iter()
                    .map(|s| s.function.clone())
                    .collect();
                *scenarios = loaded;

                vms.clear();
                snapshots.clear();

                if let Some(bundle) = kp_bundle {
                    users_lock.clear();
                    for (scn, kps) in bundle {
                        users_lock.insert(scn, kps);
                    }
                }

                let mut replay_errors: Vec<String> = Vec::new();
                let scenario_names: Vec<String> = scenarios.names().to_vec();
                for scn_name in &scenario_names {
                    let mut vm = match ilold_solana_core::execute::VmHost::boot(
                        solana.program_artifacts.clone(),
                    ) {
                        Ok(v) => v,
                        Err(e) => {
                            replay_errors.push(format!("boot {scn_name}: {e:?}"));
                            continue;
                        }
                    };
                    let scn_users = users_lock
                        .entry(scn_name.clone())
                        .or_insert_with(std::collections::HashMap::new);
                    const REPLAY_LAMPORTS: u64 = 10_000_000_000;
                    for kp in scn_users.values() {
                        use solana_keypair::Signer;
                        let _ = vm.airdrop(kp.pubkey(), REPLAY_LAMPORTS);
                    }
                    let mut stack: Vec<ilold_solana_core::execute::StateSnapshot> = Vec::new();
                    let session = match scenarios.get_mut(scn_name) {
                        Some(s) => s,
                        None => continue,
                    };
                    let steps_clone = session.steps.clone();
                    for (idx, step) in steps_clone.iter().enumerate() {
                        let payload = match &step.call_payload {
                            Some(p) => p,
                            None => continue,
                        };
                        let ix_name = payload.get("ix").and_then(|v| v.as_str()).unwrap_or("");
                        let args = payload.get("args").cloned().unwrap_or(Value::Null);
                        let accounts: std::collections::HashMap<String, String> = payload
                            .get("accounts")
                            .and_then(|v| v.as_object())
                            .map(|m| {
                                m.iter()
                                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                                    .collect()
                            })
                            .unwrap_or_default();
                        let signers: Vec<String> = payload
                            .get("signers")
                            .and_then(|v| v.as_array())
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        let pre = vm.snapshot_state();
                        let res = ilold_solana_core::exploration::execute::execute_call(
                            &program,
                            ix_name,
                            args,
                            accounts,
                            signers,
                            scn_users,
                            session,
                            &mut vm,
                            "load-replay",
                        );
                        if matches!(res, ilold_solana_core::exploration::SolanaCommandResult::StepAdded { .. }) {
                            stack.push(pre);
                            session.steps.pop();
                        } else if let ilold_solana_core::exploration::SolanaCommandResult::Error { message } = res {
                            replay_errors.push(format!("{scn_name}#{idx}: {message}"));
                        }
                    }
                    vms.insert(scn_name.clone(), vm);
                    snapshots.insert(scn_name.clone(), stack);
                }

                if replay_errors.is_empty() {
                    SolanaCommandResult::SessionLoaded {
                        program: prog,
                        steps: step_names,
                    }
                } else {
                    SolanaCommandResult::Error {
                        message: format!(
                            "loaded session but {} step(s) failed to replay: {}",
                            replay_errors.len(),
                            replay_errors.join("; ")
                        ),
                    }
                }
            }
            Err(message) => SolanaCommandResult::Error { message },
        };
        let active_after = scenarios.active().to_string();
        for patch in canvas_patches_from_solana(&result, &active_after, &[]) {
            state.session_tx.send(patch).ok();
        }
        return Ok(Json(serde_json::to_value(result).unwrap_or(Value::Null)));
    }

    let mut scenarios = state.scenarios.write().unwrap();
    let mut vms = solana.vms.write().unwrap();
    let mut users = solana.users.write().unwrap();
    let mut snapshots = solana.step_snapshots.write().unwrap();
    let active_scenario = scenarios.active().to_string();

    let vm = vms.get_mut(&active_scenario).ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("VM for scenario '{active_scenario}' missing"),
    ))?;
    let scenario_users = users.get_mut(&active_scenario).ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("users registry for scenario '{active_scenario}' missing"),
    ))?;
    let stack = snapshots
        .entry(active_scenario.clone())
        .or_insert_with(Vec::new);
    let session = scenarios.active_session_mut();

    let pre_call_snapshot = match &command {
        SolanaCommand::Call { .. } => Some(vm.snapshot_state()),
        _ => None,
    };

    let result = match command {
        SolanaCommand::Funcs => execute_funcs(&program),
        SolanaCommand::State => execute_state(session, &program, vm),
        SolanaCommand::Session => execute_session(session, &program, &active_scenario),
        SolanaCommand::Users => execute_users(scenario_users, vm),
        SolanaCommand::UsersNew { name, lamports } => {
            execute_users_new(name, lamports, scenario_users, vm)
        }
        SolanaCommand::Airdrop { user, lamports } => {
            execute_airdrop(&user, lamports, scenario_users, vm)
        }
        SolanaCommand::TimeWarp { delta_seconds } => execute_time_warp(delta_seconds, vm),
        SolanaCommand::Pda { instruction } => execute_pda(&program, &instruction),
        SolanaCommand::Inspect { pubkey } => execute_inspect(&program, vm, &pubkey),
        SolanaCommand::Call {
            ix,
            args,
            accounts,
            signers,
        } => execute_call(
            &program,
            &ix,
            args,
            accounts,
            signers,
            scenario_users,
            session,
            vm,
            &timestamp,
        ),
        SolanaCommand::Back => {
            let r = execute_back(session);
            if matches!(r, SolanaCommandResult::StepRemoved { .. }) {
                if let Some(snap) = stack.pop() {
                    if let Err(e) = vm.restore_state(snap) {
                        return Ok(Json(serde_json::to_value(SolanaCommandResult::Error {
                            message: format!("Back: rewind VM failed: {e:?}"),
                        }).unwrap_or(Value::Null)));
                    }
                }
            }
            r
        }
        SolanaCommand::Clear => {
            if let Some(genesis) = stack.first().cloned() {
                if let Err(e) = vm.restore_state(genesis) {
                    return Ok(Json(serde_json::to_value(SolanaCommandResult::Error {
                        message: format!("Clear: rewind VM failed: {e:?}"),
                    }).unwrap_or(Value::Null)));
                }
            }
            stack.clear();
            execute_clear(session)
        }
        SolanaCommand::Finding {
            severity,
            title,
            description,
            recommendation,
        } => execute_finding(session, severity, title, description, recommendation, &timestamp),
        SolanaCommand::Note { text } => execute_note(session, &text, &timestamp),
        SolanaCommand::Status { ix, status } => {
            execute_status(session, &program, &ix, status, &timestamp)
        }
        SolanaCommand::Step { index } => execute_step(session, index, &program),
        SolanaCommand::Findings => execute_findings_list(session),
        SolanaCommand::Export { metadata } => {
            let names: Vec<String> = scenarios.names().to_vec();
            let entries: Vec<(&str, &ilold_session_core::exploration::session::ExplorationSession)> =
                names
                    .iter()
                    .filter_map(|n| scenarios.get(n).map(|s| (n.as_str(), s)))
                    .collect();
            execute_export(entries, &active_scenario, &program, metadata.as_ref())
        }
        SolanaCommand::Who { account_type } => execute_who(&program, &account_type),
        SolanaCommand::Timeline { pubkey } => {
            execute_timeline(session, &program, &pubkey, &active_scenario, scenario_users)
        }
        SolanaCommand::Info { ix } => execute_info(&program, &ix),
        SolanaCommand::Coupling => execute_coupling(&program),
        SolanaCommand::Vars => execute_vars(&program),
        SolanaCommand::Coverage => execute_coverage(&program, session, &active_scenario),
        SolanaCommand::SaveSession { .. }
        | SolanaCommand::LoadSession { .. }
        | SolanaCommand::Scenario { .. } => unreachable!("handled above"),
    };

    if let (Some(snap), SolanaCommandResult::StepAdded { .. }) = (pre_call_snapshot, &result) {
        stack.push(snap);
    }

    let cpi_targets = match &result {
        SolanaCommandResult::StepAdded { .. } => scenarios
            .active_session()
            .steps
            .last()
            .and_then(|s| s.runtime_trace.as_ref())
            .and_then(|v| serde_json::from_value::<ilold_session_core::runtime_trace::RuntimeTrace>(v.clone()).ok())
            .map(|t| ilold_solana_core::overlay::extract_cpi_programs(&t))
            .unwrap_or_default(),
        _ => Vec::new(),
    };

    for patch in canvas_patches_from_solana(&result, &active_scenario, &cpi_targets) {
        state.session_tx.send(patch).ok();
    }
    Ok(Json(serde_json::to_value(result).unwrap_or(Value::Null)))
}

fn solana_scenario_action(
    scenarios: &mut ScenarioStore,
    solana: &crate::state::SolanaState,
    sub: SharedScenarioAction,
    active_before: &str,
    timestamp: &str,
    program_name: &str,
) -> SolanaCommandResult {
    match sub {
        SharedScenarioAction::New { name } => {
            if let Err(e) = ilold_session_core::exploration::scenario::validate_scenario_name(&name)
            {
                return SolanaCommandResult::Error { message: e };
            }
            if scenarios.contains(&name) {
                return SolanaCommandResult::Error {
                    message: format!("scenario '{name}' already exists"),
                };
            }
            let session = ExplorationSession::new(program_name, "ilold");
            let fresh_vm = match ilold_solana_core::execute::VmHost::boot(
                solana.program_artifacts.clone(),
            ) {
                Ok(v) => v,
                Err(e) => {
                    return SolanaCommandResult::Error {
                        message: format!("boot VM for '{name}': {e:?}"),
                    };
                }
            };
            scenarios.insert(name.clone(), session);
            solana
                .vms
                .write()
                .unwrap()
                .insert(name.clone(), fresh_vm);
            solana
                .users
                .write()
                .unwrap()
                .insert(name.clone(), std::collections::HashMap::new());
            SolanaCommandResult::ScenarioCreated { name }
        }
        SharedScenarioAction::List => {
            let active = scenarios.active().to_string();
            let items = scenarios
                .names()
                .iter()
                .map(|n| ScenarioInfo {
                    name: n.clone(),
                    active: n == &active,
                    step_count: scenarios.get(n).map(|s| s.steps.len()).unwrap_or(0),
                })
                .collect();
            SolanaCommandResult::ScenarioList { items }
        }
        SharedScenarioAction::Switch { name } => {
            let from = scenarios.active().to_string();
            if name == from {
                return SolanaCommandResult::ScenarioSwitched { from, to: name };
            }
            match scenarios.set_active(name.clone()) {
                Ok(()) => SolanaCommandResult::ScenarioSwitched { from, to: name },
                Err(e) => SolanaCommandResult::Error { message: e },
            }
        }
        SharedScenarioAction::Fork { name, at_step } => {
            if let Err(e) = ilold_session_core::exploration::scenario::validate_scenario_name(&name)
            {
                return SolanaCommandResult::Error { message: e };
            }
            if scenarios.contains(&name) {
                return SolanaCommandResult::Error {
                    message: format!("scenario '{name}' already exists"),
                };
            }
            let from = active_before.to_string();
            let mut cloned = scenarios.active_session().clone();
            cloned.reset_scenario_local_observations();
            let len = cloned.steps.len();
            let effective = match at_step {
                None => len,
                Some(n) if n > len => {
                    let noun = if len == 1 { "step" } else { "steps" };
                    return SolanaCommandResult::Error {
                        message: format!(
                            "cannot fork at step {n}: only {len} {noun} in active scenario"
                        ),
                    };
                }
                Some(n) => {
                    cloned.steps.truncate(n);
                    n
                }
            };
            cloned.forked_from = Some(ForkOrigin {
                scenario: from.clone(),
                at_step: effective,
            });
            cloned.journal.record(JournalEntry::BranchCreated {
                from_function: from.clone(),
                branch_function: name.clone(),
                timestamp: timestamp.to_string(),
            });

            let mut vms = solana.vms.write().unwrap();
            let snap = match vms.get(&from) {
                Some(vm) => vm.snapshot(),
                None => {
                    return SolanaCommandResult::Error {
                        message: format!("VM for scenario '{from}' missing"),
                    };
                }
            };
            let mut new_vm = match ilold_solana_core::execute::VmHost::restore(snap) {
                Ok(v) => v,
                Err(e) => {
                    return SolanaCommandResult::Error {
                        message: format!("restore VM for '{name}': {e:?}"),
                    };
                }
            };

            let mut snapshots = solana.step_snapshots.write().unwrap();
            let origin_stack = snapshots.entry(from.clone()).or_insert_with(Vec::new);
            let cloned_stack: Vec<ilold_solana_core::execute::StateSnapshot> =
                origin_stack.iter().take(effective).cloned().collect();
            if let Some(rewind_to) = origin_stack.get(effective) {
                if let Err(e) = new_vm.restore_state(rewind_to.clone()) {
                    return SolanaCommandResult::Error {
                        message: format!("rewind branch VM to step {effective}: {e:?}"),
                    };
                }
            }

            let mut users = solana.users.write().unwrap();
            let cloned_users: std::collections::HashMap<String, Keypair> = users
                .get(&from)
                .map(|m| {
                    m.iter()
                        .map(|(k, kp)| (k.clone(), kp.insecure_clone()))
                        .collect()
                })
                .unwrap_or_default();

            scenarios.insert(name.clone(), cloned);
            vms.insert(name.clone(), new_vm);
            users.insert(name.clone(), cloned_users);
            snapshots.insert(name.clone(), cloned_stack);
            SolanaCommandResult::ScenarioForked {
                from,
                to: name,
                at_step: effective,
            }
        }
        SharedScenarioAction::Delete { name } => {
            if name == scenarios.active() {
                return SolanaCommandResult::Error {
                    message: "cannot delete active scenario — switch first".into(),
                };
            }
            if scenarios.len() == 1 {
                return SolanaCommandResult::Error {
                    message: "cannot delete the only remaining scenario".into(),
                };
            }
            if !scenarios.contains(&name) {
                return SolanaCommandResult::Error {
                    message: format!("scenario '{name}' does not exist"),
                };
            }
            scenarios.remove(&name);
            solana.vms.write().unwrap().remove(&name);
            solana.users.write().unwrap().remove(&name);
            solana.step_snapshots.write().unwrap().remove(&name);
            SolanaCommandResult::ScenarioDeleted { name }
        }
    }
}

pub async fn get_session_step_trace(
    State(state): State<Arc<AppState>>,
    Path(step_index): Path<usize>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let scenarios_guard = state.scenarios.read().unwrap();
    let session = scenarios_guard.active_session();

    let step = session.steps.get(step_index)
        .ok_or((StatusCode::NOT_FOUND, format!("step {} not found", step_index)))?;

    let trace = step.runtime_trace.clone()
        .ok_or((
            StatusCode::NOT_FOUND,
            format!("step {} has no persisted runtime trace", step_index),
        ))?;

    Ok(Json(trace))
}

#[derive(Serialize)]
pub struct ScenarioSnapshot {
    pub name: String,
    pub steps: Vec<SessionStepView>,
    pub forked_from: Option<ForkOrigin>,
}

#[derive(Serialize)]
pub struct SessionStepView {
    pub function: String,
    pub step_index: usize,
}

#[derive(Serialize)]
pub struct AllScenariosResponse {
    pub active: String,
    pub scenarios: Vec<ScenarioSnapshot>,
}

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

pub async fn get_all_scenarios(
    State(state): State<Arc<AppState>>,
) -> Result<Json<AllScenariosResponse>, (StatusCode, String)> {
    let guard = state.scenarios.read().unwrap();
    let active = guard.active().to_string();
    let mut scenarios: Vec<ScenarioSnapshot> = Vec::with_capacity(guard.len());
    for name in guard.names() {
        let Some(session) = guard.get(name) else { continue };
        let steps = session
            .steps
            .iter()
            .enumerate()
            .map(|(idx, step)| SessionStepView {
                function: step.function.clone(),
                step_index: idx,
            })
            .collect();
        scenarios.push(ScenarioSnapshot {
            name: name.clone(),
            steps,
            forked_from: session.forked_from.clone(),
        });
    }
    Ok(Json(AllScenariosResponse { active, scenarios }))
}
