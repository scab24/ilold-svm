use std::collections::HashMap;

use ilold_session_core::exploration::access::AccessLevel;
use ilold_session_core::exploration::canvas::CanvasPatch;
use ilold_session_core::exploration::scenario::{ScenarioAction, ScenarioEvent, ScenarioInfo};
use ilold_session_core::journal::types::{ReviewStatus, Severity};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::overlay::RuntimeOverlay;
use crate::view::{AccountView, ArgView, CouplingPair, FieldView, IxView};

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
        #[serde(default)]
        recommendation: Option<String>,
    },
    Note {
        text: String,
    },
    Status {
        ix: String,
        status: ReviewStatus,
    },
    /// Persist the active scenario store. Backwards compatible with the legacy
    /// unit form `"SaveSession"` (no embedded keypairs).
    #[serde(alias = "SaveSession")]
    SaveSession {
        /// SDD-03: when true, the resulting JSON also embeds the per-scenario
        /// user keypairs in plaintext so a future LoadSession reproduces the
        /// same pubkeys (and any PDAs derived from them). Default false keeps
        /// the original "save the timeline shape" behaviour.
        #[serde(default)]
        with_keypairs: bool,
    },
    LoadSession {
        json: String,
    },
    Scenario {
        sub: ScenarioAction,
    },
    Step {
        index: usize,
    },
    Findings,
    Export {
        #[serde(default)]
        metadata: Option<ilold_session_core::journal::export::AuditMetadata>,
    },
    Who {
        account_type: String,
    },
    Timeline {
        pubkey: String,
    },
    /// Detail of a single instruction — args (typed), accounts with badges,
    /// PDAs with seeds, discriminator hex, admin-gated bool.
    Info {
        ix: String,
    },
    /// Pairs of instructions that share at least one writable account.
    Coupling,
    /// Account-type catalogue (`vars` in the REPL): name + discriminator +
    /// fields. Slice of `ProgramView::accounts`.
    Vars,
    /// Aggregated runtime metrics (calls, failures, CU, CPI edges) over the
    /// active scenario. Backend-only computation; clients consume the typed
    /// `RuntimeOverlay` payload.
    Coverage,
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
        /// Always None for StepAdded — kept for backwards-compat with clients
        /// that read this field. Failed Calls now produce `CallFailed` so the
        /// scenario timeline only contains transactions that actually mutated
        /// state (Solidity-aligned model).
        #[serde(default)]
        error: Option<String>,
    },
    /// The VM rejected the Call (Anchor constraint, custom `require!`, etc.).
    /// No step is appended to the session and no canvas broadcast is emitted
    /// — the scenario timeline stays clean. The CLI prints the error + logs
    /// inline so the auditor sees exactly what happened, and they can record
    /// the attempt manually with `note` or `finding` if it is worth keeping.
    CallFailed {
        instruction: String,
        logs_excerpt: Vec<String>,
        compute_units: u64,
        error: String,
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
    StepDetail {
        step_index: usize,
        instruction: String,
        runtime_trace: Option<Value>,
        diff_summary: Vec<StepDiffSummary>,
    },
    FindingsList {
        items: Vec<FindingSummary>,
    },
    Exported {
        markdown: String,
        bytes: usize,
    },
    WhoList {
        account_type: String,
        instructions: Vec<WhoEntry>,
        #[serde(default)]
        query_kind: WhoQueryKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        field_owner: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        field_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        owner_fields: Option<Vec<FieldView>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ix_args: Option<Vec<ArgView>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ix_discriminator_hex: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ix_accounts: Option<Vec<WhoIxAccount>>,
    },
    TimelineView {
        pubkey: String,
        label: Option<String>,
        entries: Vec<TimelineEntry>,
    },
    /// Detail for a single instruction, sliced from `ProgramView`.
    IxInfo {
        ix: IxView,
        admin_gated: bool,
    },
    CouplingList {
        pairs: Vec<CouplingPair>,
    },
    AccountTypes {
        accounts: Vec<AccountView>,
    },
    Coverage {
        overlay: RuntimeOverlay,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepDiffSummary {
    pub address: String,
    pub name: Option<String>,
    pub lamports_delta: i128,
    pub data_changed: bool,
    /// Anchor-decoded snapshot of the account before the Call ran. None when
    /// the discriminator did not match a known type (e.g. system accounts).
    #[serde(default)]
    pub decoded_before: Option<Value>,
    /// Anchor-decoded snapshot after the Call. Same caveat as `decoded_before`.
    #[serde(default)]
    pub decoded_after: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingSummary {
    pub id: String,
    pub severity: String,
    pub title: String,
    pub description: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoEntry {
    pub instruction: String,
    pub account_field: String,
    pub writable: bool,
    pub signer: bool,
    /// Resolved Anchor account type (e.g. "Pool"). None for system / sysvar /
    /// program / unknown accounts. Lets the renderer show "(as pool: Pool)".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_type: Option<String>,
    /// Args of the instruction this entry references. Useful when the auditor
    /// landed on this entry via an AccountType or Field query and wants to see
    /// what knobs each ix exposes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ix_args: Option<Vec<ArgView>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum WhoQueryKind {
    #[default]
    AccountType,
    Field,
    Instruction,
    NotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoIxAccount {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_type: Option<String>,
    pub writable: bool,
    pub signer: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<FieldView>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub step_index: usize,
    pub instruction: String,
    pub scenario: String,
    pub lamports_delta: i128,
    pub data_changed: bool,
    pub before_decoded: Option<Value>,
    pub after_decoded: Option<Value>,
}

pub fn canvas_patches_from_solana(
    result: &SolanaCommandResult,
    active_scenario: &str,
    cpi_targets: &[String],
) -> Vec<CanvasPatch> {
    use ilold_session_core::exploration::canvas::RuntimeMeta;
    match result {
        SolanaCommandResult::StepAdded {
            instruction,
            step_index,
            logs_excerpt,
            account_diffs_count,
            compute_units,
            error,
        } => vec![
            // AddNode first, OverlayUpdate after — clients that paint badges
            // off the canvas node need the node to exist before the delta
            // arrives.
            CanvasPatch::AddNode {
                scenario: active_scenario.to_string(),
                function: instruction.clone(),
                access: AccessLevel::Public,
                step_index: *step_index,
                runtime: Some(RuntimeMeta {
                    compute_units: *compute_units,
                    diffs_count: *account_diffs_count,
                    logs_excerpt: logs_excerpt.clone(),
                    error: error.clone(),
                    trace: None,
                }),
            },
            CanvasPatch::OverlayUpdate {
                scenario: active_scenario.to_string(),
                ix_name: instruction.clone(),
                calls_added: 1,
                failed_added: 0,
                cu: Some(*compute_units),
                cpi_targets_added: cpi_targets.to_vec(),
            },
        ],
        SolanaCommandResult::CallFailed {
            instruction,
            compute_units,
            ..
        } => vec![CanvasPatch::OverlayUpdate {
            scenario: active_scenario.to_string(),
            ix_name: instruction.clone(),
            calls_added: 0,
            failed_added: 1,
            cu: Some(*compute_units),
            cpi_targets_added: Vec::new(),
        }],
        SolanaCommandResult::StepRemoved { .. } => vec![CanvasPatch::RemoveLastNode {
            scenario: active_scenario.to_string(),
        }],
        SolanaCommandResult::Cleared => vec![CanvasPatch::ClearAll {
            scenario: active_scenario.to_string(),
        }],
        SolanaCommandResult::ScenarioCreated { name } => vec![CanvasPatch::ScenarioEvent(
            ScenarioEvent::Created { name: name.clone() },
        )],
        SolanaCommandResult::ScenarioSwitched { from, to } => {
            if from == to {
                Vec::new()
            } else {
                vec![CanvasPatch::ScenarioEvent(ScenarioEvent::Switched {
                    from: from.clone(),
                    to: to.clone(),
                })]
            }
        }
        SolanaCommandResult::ScenarioDeleted { name } => vec![CanvasPatch::ScenarioEvent(
            ScenarioEvent::Deleted { name: name.clone() },
        )],
        SolanaCommandResult::ScenarioForked { from, to, at_step } => {
            vec![CanvasPatch::ScenarioEvent(ScenarioEvent::Forked {
                from: from.clone(),
                to: to.clone(),
                at_step: *at_step,
            })]
        }
        SolanaCommandResult::SessionLoaded { .. } => vec![CanvasPatch::ScenarioEvent(
            ScenarioEvent::Reloaded {
                active: active_scenario.to_string(),
            },
        )],
        SolanaCommandResult::UserCreated { .. } | SolanaCommandResult::Airdropped { .. } => {
            vec![CanvasPatch::SolanaUsersChanged {
                scenario: active_scenario.to_string(),
            }]
        }
        _ => Vec::new(),
    }
}
