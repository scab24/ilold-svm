use std::collections::HashMap;

use ilold_session_core::exploration::access::AccessLevel;
use ilold_session_core::exploration::canvas::CanvasPatch;
use ilold_session_core::exploration::scenario::{ScenarioAction, ScenarioEvent, ScenarioInfo};
use ilold_session_core::journal::types::{ReviewStatus, Severity};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SolanaCommand {
    Call {
        ix: String,
        #[serde(default)]
        args: Value,
        #[serde(default)]
        accounts: HashMap<String, String>,
        #[serde(default)]
        signers: Vec<String>,
    },
    Back,
    Clear,
    Funcs,
    State,
    Session,
    Users,
    UsersNew {
        name: String,
        #[serde(default = "default_initial_lamports")]
        lamports: u64,
    },
    Airdrop {
        user: String,
        lamports: u64,
    },
    TimeWarp {
        delta_seconds: i64,
    },
    Pda {
        instruction: String,
    },
    Inspect {
        pubkey: String,
    },
    Finding {
        severity: Severity,
        title: String,
        description: String,
    },
    Note {
        text: String,
    },
    Status {
        ix: String,
        status: ReviewStatus,
    },
    SaveSession,
    LoadSession {
        json: String,
    },
    Scenario {
        sub: ScenarioAction,
    },
}

fn default_initial_lamports() -> u64 {
    10_000_000_000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionEntry {
    pub name: String,
    pub args_count: usize,
    pub accounts_count: usize,
    pub has_pdas: bool,
    pub signers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSummary {
    pub label: String,
    pub pubkey: String,
    pub owner: String,
    pub lamports: u64,
    pub decoded: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserEntry {
    pub name: String,
    pub pubkey: String,
    pub lamports: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdaEntry {
    pub account_name: String,
    pub seeds: Vec<String>,
    pub program: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SolanaCommandResult {
    StepAdded {
        step_index: usize,
        instruction: String,
        logs_excerpt: Vec<String>,
        account_diffs_count: usize,
        compute_units: u64,
    },
    StepRemoved {
        remaining: usize,
    },
    Cleared,
    InstructionList {
        items: Vec<InstructionEntry>,
    },
    StateView {
        accounts: Vec<AccountSummary>,
    },
    SessionView {
        program: String,
        scenario: String,
        steps: Vec<String>,
        findings_count: usize,
    },
    UserList {
        users: Vec<UserEntry>,
    },
    UserCreated {
        name: String,
        pubkey: String,
        lamports: u64,
    },
    Airdropped {
        name: String,
        pubkey: String,
        total_lamports: u64,
    },
    TimeWarped {
        unix_timestamp: i64,
        slot: u64,
    },
    PdaList {
        instruction: String,
        pdas: Vec<PdaEntry>,
    },
    AccountInspected {
        pubkey: String,
        owner: String,
        lamports: u64,
        data_len: usize,
        decoded: Option<Value>,
    },
    FindingAdded {
        id: String,
    },
    NoteAdded,
    StatusUpdated,
    SessionSaved {
        json: String,
    },
    SessionLoaded {
        program: String,
        steps: Vec<String>,
    },
    ScenarioList {
        items: Vec<ScenarioInfo>,
    },
    ScenarioCreated {
        name: String,
    },
    ScenarioSwitched {
        from: String,
        to: String,
    },
    ScenarioForked {
        from: String,
        to: String,
        at_step: usize,
    },
    ScenarioDeleted {
        name: String,
    },
    Error {
        message: String,
    },
}

pub fn canvas_patch_from_solana(
    result: &SolanaCommandResult,
    active_scenario: &str,
) -> Option<CanvasPatch> {
    match result {
        SolanaCommandResult::StepAdded { instruction, step_index, .. } => {
            Some(CanvasPatch::AddNode {
                scenario: active_scenario.to_string(),
                function: instruction.clone(),
                access: AccessLevel::Public,
                step_index: *step_index,
            })
        }
        SolanaCommandResult::StepRemoved { .. } => Some(CanvasPatch::RemoveLastNode {
            scenario: active_scenario.to_string(),
        }),
        SolanaCommandResult::Cleared => Some(CanvasPatch::ClearAll {
            scenario: active_scenario.to_string(),
        }),
        SolanaCommandResult::ScenarioCreated { name } => Some(CanvasPatch::ScenarioEvent(
            ScenarioEvent::Created { name: name.clone() },
        )),
        SolanaCommandResult::ScenarioSwitched { from, to } => {
            if from == to {
                None
            } else {
                Some(CanvasPatch::ScenarioEvent(ScenarioEvent::Switched {
                    from: from.clone(),
                    to: to.clone(),
                }))
            }
        }
        SolanaCommandResult::ScenarioDeleted { name } => Some(CanvasPatch::ScenarioEvent(
            ScenarioEvent::Deleted { name: name.clone() },
        )),
        SolanaCommandResult::ScenarioForked { from, to, at_step } => Some(
            CanvasPatch::ScenarioEvent(ScenarioEvent::Forked {
                from: from.clone(),
                to: to.clone(),
                at_step: *at_step,
            }),
        ),
        SolanaCommandResult::SessionLoaded { .. } => Some(CanvasPatch::ScenarioEvent(
            ScenarioEvent::Reloaded {
                active: active_scenario.to_string(),
            },
        )),
        SolanaCommandResult::UserCreated { .. } | SolanaCommandResult::Airdropped { .. } => {
            Some(CanvasPatch::SolanaUsersChanged {
                scenario: active_scenario.to_string(),
            })
        }
        _ => None,
    }
}
