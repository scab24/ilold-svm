use std::collections::HashMap;

use anchor_lang_idl::types::IdlTypeDefTy;
use ilold_session_core::exploration::session::ExplorationSession;
use ilold_session_core::journal::types::{Finding, JournalEntry, ReviewStatus, Severity};
use serde_json::Value;
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signer::Signer;

use crate::decode::borsh::decode_defined_fields;
use crate::execute::VmHost;
use crate::model::{AccountTypeDef, ProgramDef};
use crate::view::describe_seed_view;

use super::add_step::add_solana_step;
use super::commands::{
    AccountSummary, FindingSummary, InstructionEntry, PdaEntry, SolanaCommandResult,
    StepDiffSummary, TimelineEntry, UserEntry, WhoEntry, WhoIxAccount, WhoQueryKind,
};

const DEFAULT_USER_LAMPORTS: u64 = 10_000_000_000;

pub fn execute_funcs(program: &ProgramDef) -> SolanaCommandResult {
    let view = program.compute_view();
    let items = view
        .instructions
        .iter()
        .map(|ix| InstructionEntry {
            name: ix.name.clone(),
            args_count: ix.args.len(),
            accounts_count: ix.accounts.len(),
            has_pdas: ix.accounts.iter().any(|a| a.pda.is_some()),
            signers: ix
                .accounts
                .iter()
                .filter(|a| a.signer)
                .map(|a| a.name.clone())
                .collect(),
        })
        .collect();
    SolanaCommandResult::InstructionList { items }
}

pub fn execute_info(program: &ProgramDef, ix_name: &str) -> SolanaCommandResult {
    let view = program.compute_view();
    let ix = match view.instructions.iter().find(|i| i.name == ix_name) {
        Some(i) => i.clone(),
        None => {
            return SolanaCommandResult::Error {
                message: format!("instruction '{ix_name}' not found"),
            };
        }
    };
    let admin_gated = view
        .admin_gated
        .as_ref()
        .map(|set| set.contains(ix_name))
        .unwrap_or(false);
    SolanaCommandResult::IxInfo { ix, admin_gated }
}

pub fn execute_coupling(program: &ProgramDef) -> SolanaCommandResult {
    let view = program.compute_view();
    SolanaCommandResult::CouplingList {
        pairs: view.state_coupling.unwrap_or_default(),
    }
}

pub fn execute_vars(program: &ProgramDef) -> SolanaCommandResult {
    let view = program.compute_view();
    SolanaCommandResult::AccountTypes {
        accounts: view.accounts,
    }
}

pub fn execute_coverage(
    program: &ProgramDef,
    session: &ExplorationSession,
    scenario: &str,
) -> SolanaCommandResult {
    let mut overlay = crate::overlay::RuntimeOverlay::from_session(session);
    if overlay.program.is_empty() {
        overlay.program = program.name.clone();
    }
    overlay.scenario = scenario.to_string();
    SolanaCommandResult::Coverage { overlay }
}

pub fn execute_users(users: &HashMap<String, Keypair>, vm: &VmHost) -> SolanaCommandResult {
    let mut entries: Vec<UserEntry> = users
        .iter()
        .map(|(name, kp)| {
            let pk = kp.pubkey();
            UserEntry {
                name: name.clone(),
                pubkey: pk.to_string(),
                lamports: vm.balance(&pk),
            }
        })
        .collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    SolanaCommandResult::UserList { users: entries }
}

pub fn execute_state(
    session: &ExplorationSession,
    program: &ProgramDef,
    vm: &VmHost,
) -> SolanaCommandResult {
    let mut seen: std::collections::BTreeMap<String, AccountSummary> =
        std::collections::BTreeMap::new();

    for (idx, step) in session.steps.iter().enumerate() {
        let trace = match step.runtime_trace.as_ref() {
            Some(t) => t,
            None => continue,
        };
        let diffs = trace.get("account_diffs").and_then(|v| v.as_array());
        let diffs = match diffs {
            Some(d) => d,
            None => continue,
        };
        for diff in diffs {
            let address = diff.get("address").and_then(|v| v.as_str()).unwrap_or("");
            if address.is_empty() {
                continue;
            }
            let label = diff
                .get("name")
                .and_then(|v| v.as_str())
                .map(|n| format!("{n}#{idx}"))
                .unwrap_or_else(|| format!("acc#{idx}"));
            let pk: Address = match address.parse() {
                Ok(p) => p,
                Err(_) => continue,
            };
            let acc = match vm.svm().get_account(&pk) {
                Some(a) => a,
                None => continue,
            };
            let decoded = decode_account_bytes(&acc.data, &program.account_types, &program.types);
            seen.insert(
                address.to_string(),
                AccountSummary {
                    label,
                    pubkey: address.to_string(),
                    owner: acc.owner.to_string(),
                    lamports: acc.lamports,
                    decoded,
                },
            );
        }
    }

    SolanaCommandResult::StateView {
        accounts: seen.into_values().collect(),
    }
}

pub fn execute_session(
    session: &ExplorationSession,
    program: &ProgramDef,
    active_scenario: &str,
) -> SolanaCommandResult {
    SolanaCommandResult::SessionView {
        program: program.name.clone(),
        scenario: active_scenario.to_string(),
        steps: session.steps.iter().map(|s| s.function.clone()).collect(),
        findings_count: session.journal.findings.len(),
    }
}

pub fn execute_pda(program: &ProgramDef, instruction: &str) -> SolanaCommandResult {
    let view = program.compute_view();
    let ix = match view.instructions.iter().find(|i| i.name == instruction) {
        Some(i) => i,
        None => {
            return SolanaCommandResult::Error {
                message: format!("instruction '{instruction}' not found"),
            };
        }
    };

    let self_program_id = view.program_id.clone();
    let pdas: Vec<PdaEntry> = ix
        .accounts
        .iter()
        .filter_map(|acc| {
            let pda = acc.pda.as_ref()?;
            let seeds = pda.seeds.iter().map(describe_seed_view).collect();
            let prog = pda.program.clone().unwrap_or_else(|| self_program_id.clone());
            Some(PdaEntry {
                account_name: acc.name.clone(),
                seeds,
                program: prog,
            })
        })
        .collect();

    SolanaCommandResult::PdaList {
        instruction: instruction.to_string(),
        pdas,
    }
}

pub fn execute_inspect(
    program: &ProgramDef,
    vm: &VmHost,
    pubkey: &str,
) -> SolanaCommandResult {
    let pk: Address = match pubkey.parse() {
        Ok(p) => p,
        Err(_) => {
            return SolanaCommandResult::Error {
                message: format!("invalid pubkey '{pubkey}'"),
            };
        }
    };
    let acc = match vm.svm().get_account(&pk) {
        Some(a) => a,
        None => {
            return SolanaCommandResult::Error {
                message: format!("account '{pubkey}' not found in VM"),
            };
        }
    };
    let decoded = decode_account_bytes(&acc.data, &program.account_types, &program.types);
    SolanaCommandResult::AccountInspected {
        pubkey: pubkey.to_string(),
        owner: acc.owner.to_string(),
        lamports: acc.lamports,
        data_len: acc.data.len(),
        decoded,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn execute_call(
    program: &ProgramDef,
    ix_name: &str,
    args: Value,
    accounts_input: HashMap<String, String>,
    signer_names: Vec<String>,
    users: &HashMap<String, Keypair>,
    session: &mut ExplorationSession,
    vm: &mut VmHost,
    timestamp: &str,
) -> SolanaCommandResult {
    let ix = match program.instructions.iter().find(|i| i.name == ix_name) {
        Some(i) => i.clone(),
        None => {
            return SolanaCommandResult::Error {
                message: format!("instruction '{ix_name}' not found"),
            };
        }
    };

    // Capture original inputs so LoadSession can replay this Call against a
    // fresh VM. We serialize the user-name strings (not the resolved pubkeys)
    // because user keypairs are recreated on Load — same name, same pubkey.
    let call_payload = serde_json::json!({
        "ix": ix_name,
        "args": args.clone(),
        "accounts": accounts_input.clone(),
        "signers": signer_names.clone(),
    });

    let mut accounts: HashMap<String, Address> = HashMap::new();
    for (key, raw) in accounts_input {
        if let Some(kp) = users.get(&raw) {
            accounts.insert(key, kp.pubkey());
            continue;
        }
        match raw.parse::<Address>() {
            Ok(addr) => {
                accounts.insert(key, addr);
            }
            Err(_) => {
                return SolanaCommandResult::Error {
                    message: format!(
                        "account '{key}': '{raw}' is neither a known user nor a valid pubkey"
                    ),
                };
            }
        }
    }

    let mut extra_signers: Vec<&Keypair> = Vec::new();
    for name in &signer_names {
        match users.get(name) {
            Some(kp) => extra_signers.push(kp),
            None => {
                return SolanaCommandResult::Error {
                    message: format!("signer '{name}' not found in users registry"),
                };
            }
        }
    }

    let payer_pk = vm.payer_pubkey();
    for spec in &ix.accounts {
        if !spec.signer {
            continue;
        }
        let resolved_pk = accounts
            .get(&spec.path)
            .or_else(|| accounts.get(&spec.name))
            .copied();
        let pk = match resolved_pk {
            Some(p) => p,
            None => continue,
        };
        let in_signers = extra_signers.iter().any(|kp| kp.pubkey() == pk);
        if pk != payer_pk && !in_signers {
            return SolanaCommandResult::Error {
                message: format!(
                    "account '{}' is marked signer but no matching keypair was provided",
                    spec.name
                ),
            };
        }
    }

    let outcome = match add_solana_step(
        session,
        program,
        &ix,
        vm,
        args,
        accounts,
        &extra_signers,
        timestamp,
        Some(call_payload),
    ) {
        Ok(o) => o,
        Err(e) => {
            return SolanaCommandResult::Error {
                message: format!("{e:?}"),
            }
        }
    };

    let trace = &outcome.trace;
    let logs_excerpt: Vec<String> = trace.logs.iter().take(10).cloned().collect();
    let compute_units = trace.compute_units;

    match outcome.step_index {
        Some(idx) => SolanaCommandResult::StepAdded {
            step_index: idx,
            instruction: ix.name.clone(),
            logs_excerpt,
            account_diffs_count: trace.account_diffs.len(),
            compute_units,
            error: None,
        },
        None => SolanaCommandResult::CallFailed {
            instruction: ix.name.clone(),
            logs_excerpt,
            compute_units,
            error: trace
                .error
                .clone()
                .unwrap_or_else(|| "unknown VM error".to_string()),
        },
    }
}

pub fn execute_back(session: &mut ExplorationSession) -> SolanaCommandResult {
    if session.remove_last_step() {
        SolanaCommandResult::StepRemoved {
            remaining: session.steps.len(),
        }
    } else {
        SolanaCommandResult::Error {
            message: "no steps to undo".into(),
        }
    }
}

pub fn execute_clear(session: &mut ExplorationSession) -> SolanaCommandResult {
    session.clear();
    SolanaCommandResult::Cleared
}

pub fn execute_users_new(
    name: String,
    lamports: u64,
    users: &mut HashMap<String, Keypair>,
    vm: &mut VmHost,
) -> SolanaCommandResult {
    if users.contains_key(&name) {
        return SolanaCommandResult::Error {
            message: format!("user '{name}' already exists"),
        };
    }
    let kp = Keypair::new();
    let pk = kp.pubkey();
    let funded = if lamports == 0 {
        DEFAULT_USER_LAMPORTS
    } else {
        lamports
    };
    if let Err(e) = vm.airdrop(pk, funded) {
        return SolanaCommandResult::Error {
            message: format!("airdrop failed: {e:?}"),
        };
    }
    users.insert(name.clone(), kp);
    SolanaCommandResult::UserCreated {
        name,
        pubkey: pk.to_string(),
        lamports: funded,
    }
}

pub fn execute_airdrop(
    user: &str,
    lamports: u64,
    users: &HashMap<String, Keypair>,
    vm: &mut VmHost,
) -> SolanaCommandResult {
    let kp = match users.get(user) {
        Some(k) => k,
        None => {
            return SolanaCommandResult::Error {
                message: format!("user '{user}' not found"),
            };
        }
    };
    let pk = kp.pubkey();
    if let Err(e) = vm.airdrop(pk, lamports) {
        return SolanaCommandResult::Error {
            message: format!("airdrop failed: {e:?}"),
        };
    }
    SolanaCommandResult::Airdropped {
        name: user.to_string(),
        pubkey: pk.to_string(),
        total_lamports: vm.balance(&pk),
    }
}

pub fn execute_time_warp(delta_seconds: i64, vm: &mut VmHost) -> SolanaCommandResult {
    let clock = vm.clock();
    let new_ts = clock.unix_timestamp.saturating_add(delta_seconds);
    let slot_advance = delta_seconds.max(0) as u64;
    let new_slot = clock.slot.saturating_add(slot_advance);
    vm.warp_clock(new_slot, new_ts);
    SolanaCommandResult::TimeWarped {
        unix_timestamp: new_ts,
        slot: new_slot,
    }
}

pub fn execute_finding(
    session: &mut ExplorationSession,
    severity: Severity,
    title: String,
    description: String,
    recommendation: Option<String>,
    timestamp: &str,
) -> SolanaCommandResult {
    let affected_sequence = if session.steps.is_empty() {
        None
    } else {
        Some(
            session
                .current_sequence()
                .into_iter()
                .map(|s| s.to_string())
                .collect(),
        )
    };
    // Capture the index of the most recent step so the export can render
    // "Step #N" alongside the affected function. None when no steps yet.
    let affected_step_index = if session.steps.is_empty() {
        None
    } else {
        Some(session.steps.len() - 1)
    };
    let finding = Finding {
        id: String::new(),
        severity,
        title,
        affected_function: session
            .steps
            .last()
            .map(|s| s.function.clone())
            .unwrap_or_default(),
        affected_sequence,
        description,
        notes: vec![],
        created_at: String::new(),
        affected_step_index,
        recommendation,
    };
    session.journal.add_finding(finding, timestamp);
    let id = session
        .journal
        .findings
        .last()
        .map(|f| f.id.clone())
        .unwrap_or_default();
    SolanaCommandResult::FindingAdded { id }
}

pub fn execute_note(
    session: &mut ExplorationSession,
    text: &str,
    timestamp: &str,
) -> SolanaCommandResult {
    let anchor = session.current_sequence().join(" → ");
    session.journal.record(JournalEntry::NoteAdded {
        anchor,
        content: text.into(),
        timestamp: timestamp.into(),
    });
    SolanaCommandResult::NoteAdded
}

pub fn execute_status(
    session: &mut ExplorationSession,
    program: &ProgramDef,
    ix_name: &str,
    status: ReviewStatus,
    timestamp: &str,
) -> SolanaCommandResult {
    if !program.instructions.iter().any(|i| i.name == ix_name) {
        return SolanaCommandResult::Error {
            message: format!("instruction '{ix_name}' not found in program '{}'", program.name),
        };
    }
    session.journal.record(JournalEntry::StatusChanged {
        function: ix_name.into(),
        status,
        timestamp: timestamp.into(),
    });
    SolanaCommandResult::StatusUpdated
}

fn decode_account_bytes(
    data: &[u8],
    account_types: &[AccountTypeDef],
    types: &[anchor_lang_idl::types::IdlTypeDef],
) -> Option<serde_json::Value> {
    if data.len() < 8 {
        return None;
    }
    let (disc, rest) = data.split_at(8);
    let acc_def = account_types.iter().find(|a| a.discriminator == disc)?;
    let mut cursor = rest;
    match &acc_def.layout.ty {
        IdlTypeDefTy::Struct { fields } => {
            decode_defined_fields(&mut cursor, fields.as_ref(), types).ok()
        }
        _ => None,
    }
}

pub fn execute_step(
    session: &ExplorationSession,
    index: usize,
    program: &ProgramDef,
) -> SolanaCommandResult {
    let step = match session.steps.get(index) {
        Some(s) => s,
        None => {
            return SolanaCommandResult::Error {
                message: format!(
                    "step {index} out of range (session has {} steps)",
                    session.steps.len()
                ),
            };
        }
    };
    let runtime_trace = step.runtime_trace.clone();
    let diff_summary: Vec<StepDiffSummary> = runtime_trace
        .as_ref()
        .and_then(|v| v.get("account_diffs").and_then(|d| d.as_array()))
        .map(|arr| {
            arr.iter()
                .map(|d| {
                    let before_bytes: Option<Vec<u8>> = d.get("before").and_then(|v| v.as_array())
                        .map(|a| a.iter().filter_map(|b| b.as_u64().map(|n| n as u8)).collect());
                    let after_bytes: Option<Vec<u8>> = d.get("after").and_then(|v| v.as_array())
                        .map(|a| a.iter().filter_map(|b| b.as_u64().map(|n| n as u8)).collect());
                    let decoded_before = before_bytes.as_ref().and_then(|b|
                        decode_account_bytes(b, &program.account_types, &program.types));
                    let decoded_after = after_bytes.as_ref().and_then(|b|
                        decode_account_bytes(b, &program.account_types, &program.types));
                    StepDiffSummary {
                        address: d.get("address").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        name: d.get("name").and_then(|v| v.as_str()).map(String::from),
                        lamports_delta: d
                            .get("lamports_delta")
                            .and_then(|v| v.as_i64())
                            .map(|n| n as i128)
                            .unwrap_or(0),
                        data_changed: before_bytes != after_bytes,
                        decoded_before,
                        decoded_after,
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    SolanaCommandResult::StepDetail {
        step_index: index,
        instruction: step.function.clone(),
        runtime_trace,
        diff_summary,
    }
}

pub fn execute_findings_list(session: &ExplorationSession) -> SolanaCommandResult {
    let items: Vec<FindingSummary> = session
        .journal
        .findings
        .iter()
        .map(|f: &Finding| FindingSummary {
            id: f.id.clone(),
            severity: format!("{:?}", f.severity),
            title: f.title.clone(),
            description: f.description.clone(),
            created_at: f.created_at.clone(),
        })
        .collect();
    SolanaCommandResult::FindingsList { items }
}

pub fn execute_export<'a, I>(
    scenarios: I,
    active: &str,
    program: &ProgramDef,
    metadata: Option<&ilold_session_core::journal::export::AuditMetadata>,
) -> SolanaCommandResult
where
    I: IntoIterator<Item = (&'a str, &'a ExplorationSession)>,
{
    use ilold_session_core::journal::export::{
        export_markdown_multi, ProgramSection,
    };
    let scenarios: Vec<(&str, &ExplorationSession)> = scenarios.into_iter().collect();
    let prog_section = ProgramSection {
        name: program.name.clone(),
        program_id: program.program_id.to_string(),
        instructions: program.instructions.len(),
        account_types: program.account_types.len(),
    };

    // Reuse the shared markdown renderer (header + metadata + program +
    // methodology + severity matrix + findings detail). Only the per-scenario
    // step listing stays here because step records belong to ExplorationStep,
    // which is owned by ilold-session-core but printed with Solana semantics
    // (compute units, error from runtime_trace).
    let journal_pairs: Vec<(&str, &ilold_session_core::journal::types::AuditJournal)> =
        scenarios.iter().map(|(n, s)| (*n, &s.journal)).collect();
    let mut md = export_markdown_multi(
        &journal_pairs,
        Some(&prog_section),
        metadata,
        program.instructions.len(),
    );

    // Per-scenario step listing — Solana-specific (no Solidity counterpart).
    use std::fmt::Write;
    writeln!(md, "## Scenarios\n").unwrap();
    writeln!(md, "**Active**: `{active}`\n").unwrap();
    for (scn_name, session) in &scenarios {
        writeln!(md, "### `{scn_name}` — {} steps\n", session.steps.len()).unwrap();
        if session.steps.is_empty() {
            writeln!(md, "_(no steps)_\n").unwrap();
            continue;
        }
        for (i, s) in session.steps.iter().enumerate() {
            let cu = s.runtime_trace.as_ref()
                .and_then(|v| v.get("compute_units"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let err = s.runtime_trace.as_ref()
                .and_then(|v| v.get("error"))
                .and_then(|v| v.as_str());
            let mark = if err.is_some() { "FAIL" } else { "OK" };
            writeln!(md, "- **#{i}** `{}` — {} ({} CU)", s.function, mark, cu).unwrap();
            if let Some(e) = err {
                writeln!(md, "  - error: `{e}`").unwrap();
            }
        }
        writeln!(md).unwrap();
    }

    let bytes = md.len();
    SolanaCommandResult::Exported { markdown: md, bytes }
}

pub fn execute_who(
    program: &ProgramDef,
    query: &str,
) -> SolanaCommandResult {
    let view = program.compute_view();
    let raw = query.trim();

    let resolved_account_type = view
        .accounts
        .iter()
        .find(|a| a.name == raw)
        .map(|a| a.name.clone())
        .or_else(|| {
            let pascal = crate::view::snake_to_pascal(raw);
            view.accounts
                .iter()
                .find(|a| a.name == pascal)
                .map(|a| a.name.clone())
        });

    if let Some(name) = resolved_account_type {
        return who_for_account_type(&view, &name);
    }

    if let Some(ix) = view.instructions.iter().find(|i| i.name == raw) {
        return who_for_instruction(&view, ix);
    }

    if let Some((owner_name, field)) = find_field_owner(&view, raw) {
        return who_for_field(&view, raw, &owner_name, &field);
    }

    SolanaCommandResult::WhoList {
        account_type: raw.to_string(),
        instructions: Vec::new(),
        query_kind: WhoQueryKind::NotFound,
        field_owner: None,
        field_type: None,
        owner_fields: None,
        ix_args: None,
        ix_discriminator_hex: None,
        ix_accounts: None,
    }
}

fn who_for_account_type(
    view: &crate::view::ProgramView,
    type_name: &str,
) -> SolanaCommandResult {
    let mut hits: Vec<WhoEntry> = Vec::new();
    for ix in &view.instructions {
        for acc in &ix.accounts {
            let resolved = resolve_account_type(view, &acc.name);
            if resolved.as_deref() == Some(type_name) {
                hits.push(WhoEntry {
                    instruction: ix.name.clone(),
                    account_field: acc.name.clone(),
                    writable: acc.writable,
                    signer: acc.signer,
                    account_type: resolved,
                    ix_args: Some(ix.args.clone()),
                });
            }
        }
    }
    hits.sort_by(|a, b| a.instruction.cmp(&b.instruction));
    let owner_fields = view
        .accounts
        .iter()
        .find(|a| a.name == type_name)
        .map(|a| a.fields.clone());
    SolanaCommandResult::WhoList {
        account_type: type_name.to_string(),
        instructions: hits,
        query_kind: WhoQueryKind::AccountType,
        field_owner: None,
        field_type: None,
        owner_fields,
        ix_args: None,
        ix_discriminator_hex: None,
        ix_accounts: None,
    }
}

fn who_for_instruction(
    view: &crate::view::ProgramView,
    ix: &crate::view::IxView,
) -> SolanaCommandResult {
    let accounts: Vec<WhoIxAccount> = ix
        .accounts
        .iter()
        .map(|acc| {
            let resolved = resolve_account_type(view, &acc.name);
            let fields = resolved.as_ref().and_then(|t| {
                view.accounts
                    .iter()
                    .find(|a| &a.name == t)
                    .map(|a| a.fields.clone())
            });
            WhoIxAccount {
                name: acc.name.clone(),
                account_type: resolved,
                writable: acc.writable,
                signer: acc.signer,
                fields,
            }
        })
        .collect();
    SolanaCommandResult::WhoList {
        account_type: ix.name.clone(),
        instructions: Vec::new(),
        query_kind: WhoQueryKind::Instruction,
        field_owner: None,
        field_type: None,
        owner_fields: None,
        ix_args: Some(ix.args.clone()),
        ix_discriminator_hex: Some(ix.discriminator_hex.clone()),
        ix_accounts: Some(accounts),
    }
}

fn who_for_field(
    view: &crate::view::ProgramView,
    field_name: &str,
    owner: &str,
    field: &crate::view::FieldView,
) -> SolanaCommandResult {
    // Heuristic: without source-level analysis we list every ix that touches
    // the owner account-type as writable. The renderer must surface this.
    let mut hits: Vec<WhoEntry> = Vec::new();
    for ix in &view.instructions {
        for acc in &ix.accounts {
            let resolved = resolve_account_type(view, &acc.name);
            if resolved.as_deref() == Some(owner) && acc.writable {
                hits.push(WhoEntry {
                    instruction: ix.name.clone(),
                    account_field: acc.name.clone(),
                    writable: acc.writable,
                    signer: acc.signer,
                    account_type: resolved,
                    ix_args: Some(ix.args.clone()),
                });
                break;
            }
        }
    }
    hits.sort_by(|a, b| a.instruction.cmp(&b.instruction));
    let owner_fields = view
        .accounts
        .iter()
        .find(|a| a.name == owner)
        .map(|a| a.fields.clone());
    SolanaCommandResult::WhoList {
        account_type: field_name.to_string(),
        instructions: hits,
        query_kind: WhoQueryKind::Field,
        field_owner: Some(owner.to_string()),
        field_type: Some(field.ty.clone()),
        owner_fields,
        ix_args: None,
        ix_discriminator_hex: None,
        ix_accounts: None,
    }
}

fn find_field_owner(
    view: &crate::view::ProgramView,
    field_name: &str,
) -> Option<(String, crate::view::FieldView)> {
    // Stable iteration order: AccountView vector preserves IDL order, which is
    // the order ProgramDef::from_idl emitted. That is deterministic per-IDL.
    for acc in &view.accounts {
        if let Some(f) = acc.fields.iter().find(|f| f.name == field_name) {
            return Some((acc.name.clone(), f.clone()));
        }
    }
    None
}

fn resolve_account_type(
    view: &crate::view::ProgramView,
    account_name: &str,
) -> Option<String> {
    let pascal = crate::view::snake_to_pascal(account_name);
    if view.accounts.iter().any(|a| a.name == pascal) {
        return Some(pascal);
    }
    if view.accounts.iter().any(|a| a.name == account_name) {
        return Some(account_name.to_string());
    }
    None
}

pub fn execute_timeline(
    session: &ExplorationSession,
    program: &ProgramDef,
    raw_target: &str,
    active_scenario: &str,
    users: &HashMap<String, Keypair>,
) -> SolanaCommandResult {
    // The auditor types `tl alice` or `tl <pubkey>` interchangeably; normalise
    // to the on-wire pubkey before walking the diffs.
    let resolved_label = users.get(raw_target).map(|_| raw_target.to_string());
    let pubkey = match users.get(raw_target) {
        Some(kp) => kp.pubkey().to_string(),
        None => raw_target.to_string(),
    };
    let pubkey = pubkey.as_str();
    let mut entries: Vec<TimelineEntry> = Vec::new();
    let mut label: Option<String> = resolved_label;
    for (idx, step) in session.steps.iter().enumerate() {
        let trace = match &step.runtime_trace {
            Some(t) => t,
            None => continue,
        };
        let diffs = match trace.get("account_diffs").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => continue,
        };
        for d in diffs {
            let addr = d.get("address").and_then(|v| v.as_str()).unwrap_or("");
            if addr != pubkey {
                continue;
            }
            if label.is_none() {
                label = d.get("name").and_then(|v| v.as_str()).map(String::from);
            }
            let lamports_delta = d
                .get("lamports_delta")
                .and_then(|v| v.as_i64())
                .map(|n| n as i128)
                .unwrap_or(0);
            let data_changed = d
                .get("before")
                .and_then(|v| v.as_array())
                .zip(d.get("after").and_then(|v| v.as_array()))
                .map(|(b, a)| b != a)
                .unwrap_or(false);
            // Try to decode before/after using IDL discriminators.
            let decode = |bytes_v: Option<&Value>| -> Option<Value> {
                let arr = bytes_v.and_then(|v| v.as_array())?;
                let bytes: Vec<u8> = arr.iter().filter_map(|b| b.as_u64().map(|n| n as u8)).collect();
                decode_account_bytes(&bytes, &program.account_types, &program.types)
            };
            entries.push(TimelineEntry {
                step_index: idx,
                instruction: step.function.clone(),
                scenario: active_scenario.to_string(),
                lamports_delta,
                data_changed,
                before_decoded: decode(d.get("before")),
                after_decoded: decode(d.get("after")),
            });
        }
    }
    SolanaCommandResult::TimelineView {
        pubkey: pubkey.to_string(),
        label,
        entries,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idl::parse_idl;

    const STAKING_JSON: &str = include_str!(
        "../../../../tests/fixtures/solana/staking/idls/staking.json"
    );
    const LEVER_JSON: &str = include_str!("../../tests/fixtures/lever.json");

    fn staking() -> ProgramDef {
        ProgramDef::from_idl(parse_idl(STAKING_JSON).expect("parse staking"))
            .expect("build staking ProgramDef")
    }

    fn lever() -> ProgramDef {
        ProgramDef::from_idl(parse_idl(LEVER_JSON).expect("parse lever"))
            .expect("build lever ProgramDef")
    }

    fn unwrap_who(
        result: SolanaCommandResult,
    ) -> (
        String,
        Vec<WhoEntry>,
        WhoQueryKind,
        Option<String>,
        Option<Vec<crate::view::FieldView>>,
        Option<Vec<crate::exploration::commands::WhoIxAccount>>,
        Option<Vec<crate::view::ArgView>>,
    ) {
        match result {
            SolanaCommandResult::WhoList {
                account_type,
                instructions,
                query_kind,
                field_owner,
                owner_fields,
                ix_accounts,
                ix_args,
                ..
            } => (
                account_type,
                instructions,
                query_kind,
                field_owner,
                owner_fields,
                ix_accounts,
                ix_args,
            ),
            other => panic!("expected WhoList, got {other:?}"),
        }
    }

    #[test]
    fn who_resolves_account_type_pool() {
        let (target, hits, kind, owner, fields, accounts, _) =
            unwrap_who(execute_who(&staking(), "Pool"));
        assert_eq!(target, "Pool");
        assert_eq!(kind, WhoQueryKind::AccountType);
        assert!(owner.is_none());
        assert_eq!(hits.len(), 5);
        let names: Vec<_> = hits.iter().map(|w| w.instruction.as_str()).collect();
        assert_eq!(
            names,
            vec!["add_rewards", "claim_rewards", "initialize_pool", "stake", "unstake"]
        );
        // Sorted alphabetically for snapshot stability.
        assert!(hits.iter().all(|w| w.account_type.as_deref() == Some("Pool")));
        assert!(hits.iter().all(|w| w.ix_args.is_some()));
        let pool_fields = fields.expect("Pool fields populated");
        assert!(pool_fields.iter().any(|f| f.name == "total_staked"));
        assert!(accounts.is_none());
    }

    #[test]
    fn who_resolves_lowercase_account_type() {
        let (target, hits, kind, ..) = unwrap_who(execute_who(&staking(), "pool"));
        assert_eq!(target, "Pool");
        assert_eq!(kind, WhoQueryKind::AccountType);
        assert_eq!(hits.len(), 5);
    }

    #[test]
    fn who_resolves_instruction_claim_rewards() {
        let (target, hits, kind, owner, fields, accounts, args) =
            unwrap_who(execute_who(&staking(), "claim_rewards"));
        assert_eq!(target, "claim_rewards");
        assert_eq!(kind, WhoQueryKind::Instruction);
        assert!(hits.is_empty());
        assert!(owner.is_none());
        assert!(fields.is_none());
        let accs = accounts.expect("ix_accounts populated");
        assert!(accs.iter().any(|a| a.name == "pool" && a.account_type.as_deref() == Some("Pool")));
        assert!(accs
            .iter()
            .any(|a| a.name == "user_stake" && a.account_type.as_deref() == Some("UserStake")));
        // The 'user' signer maps to no account type — must not crash, must be None.
        assert!(accs
            .iter()
            .any(|a| a.name == "user" && a.account_type.is_none() && a.signer));
        assert!(args.is_some());
    }

    #[test]
    fn who_resolves_field_total_staked() {
        let (target, hits, kind, owner, fields, accounts, _) =
            unwrap_who(execute_who(&staking(), "total_staked"));
        assert_eq!(target, "total_staked");
        assert_eq!(kind, WhoQueryKind::Field);
        assert_eq!(owner.as_deref(), Some("Pool"));
        assert!(accounts.is_none());
        let pool_fields = fields.expect("owner_fields present");
        assert!(pool_fields.iter().any(|f| f.name == "total_staked"));
        // All 5 ix that touch Pool as writable.
        let names: Vec<_> = hits.iter().map(|w| w.instruction.as_str()).collect();
        assert_eq!(
            names,
            vec!["add_rewards", "claim_rewards", "initialize_pool", "stake", "unstake"]
        );
    }

    #[test]
    fn who_returns_not_found_for_unknown_query() {
        let (target, hits, kind, owner, ..) =
            unwrap_who(execute_who(&staking(), "nonexistent"));
        assert_eq!(target, "nonexistent");
        assert_eq!(kind, WhoQueryKind::NotFound);
        assert!(hits.is_empty());
        assert!(owner.is_none());
    }

    #[test]
    fn who_field_returns_no_writers_when_account_name_does_not_map() {
        // Edge case: lever declares the IDL account as `power` (snake) but the
        // type is `PowerStatus` — snake_to_pascal("power") = "Power" ≠
        // "PowerStatus". Without source-level analysis we can't bridge that gap,
        // so the heuristic must surface zero writers for `is_on` rather than
        // guessing. This is a known limitation we surface honestly.
        let (target, hits, kind, owner, fields, ..) =
            unwrap_who(execute_who(&lever(), "is_on"));
        assert_eq!(target, "is_on");
        assert_eq!(kind, WhoQueryKind::Field);
        assert_eq!(owner.as_deref(), Some("PowerStatus"));
        assert!(hits.is_empty(), "no field-name-to-type bridge available");
        assert!(fields.is_some());
    }

    #[test]
    fn who_instruction_handles_system_program_account_kind() {
        let (_, _, kind, _, _, accounts, _) =
            unwrap_who(execute_who(&lever(), "initialize"));
        assert_eq!(kind, WhoQueryKind::Instruction);
        let accs = accounts.expect("ix_accounts populated");
        let sys = accs
            .iter()
            .find(|a| a.name == "system_program")
            .expect("system_program present");
        assert!(sys.account_type.is_none());
        assert!(!sys.signer && !sys.writable);
        assert!(sys.fields.is_none());
    }
}
