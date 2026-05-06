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
use crate::model::{AccountTypeDef, ProgramDef, SeedSpec};

use super::add_step::add_solana_step;
use super::commands::{
    AccountSummary, InstructionEntry, PdaEntry, SolanaCommandResult, UserEntry,
};

const DEFAULT_USER_LAMPORTS: u64 = 10_000_000_000;

pub fn execute_funcs(program: &ProgramDef) -> SolanaCommandResult {
    let items = program
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
    let ix = match program.instructions.iter().find(|i| i.name == instruction) {
        Some(i) => i,
        None => {
            return SolanaCommandResult::Error {
                message: format!("instruction '{instruction}' not found"),
            };
        }
    };

    let pdas: Vec<PdaEntry> = ix
        .accounts
        .iter()
        .filter_map(|spec| {
            let pda_spec = spec.pda.as_ref()?;
            let seeds = pda_spec.seeds.iter().map(describe_seed).collect();
            let prog = match &pda_spec.program {
                None => program.program_id.to_string(),
                Some(SeedSpec::Const { value }) => Address::try_from(value.as_slice())
                    .map(|a| a.to_string())
                    .unwrap_or_else(|_| format!("const:{:02x?}", value)),
                Some(SeedSpec::Account { path }) => format!("account:{path}"),
                Some(SeedSpec::Arg { path, .. }) => format!("arg:{path}"),
            };
            Some(PdaEntry {
                account_name: spec.name.clone(),
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

    if let Err(e) = add_solana_step(
        session,
        program,
        &ix,
        vm,
        args,
        accounts,
        &extra_signers,
        timestamp,
    ) {
        return SolanaCommandResult::Error {
            message: format!("{e:?}"),
        };
    }

    let step_index = session.steps.len() - 1;
    let step = session.steps.last().expect("step pushed");
    let trace = step.runtime_trace.clone().unwrap_or(Value::Null);
    let logs_excerpt: Vec<String> = trace
        .get("logs")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .take(10)
                .collect()
        })
        .unwrap_or_default();
    let account_diffs_count = trace
        .get("account_diffs")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let compute_units = trace
        .get("compute_units")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    SolanaCommandResult::StepAdded {
        step_index,
        instruction: step.function.clone(),
        logs_excerpt,
        account_diffs_count,
        compute_units,
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

fn describe_seed(seed: &SeedSpec) -> String {
    match seed {
        SeedSpec::Const { value } => match std::str::from_utf8(value) {
            Ok(s) if s.chars().all(|c| c.is_ascii_graphic() || c == ' ') => format!("const:'{s}'"),
            _ => format!("const:{:02x?}", value),
        },
        SeedSpec::Account { path } => format!("account:{path}"),
        SeedSpec::Arg { path, .. } => format!("arg:{path}"),
    }
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
