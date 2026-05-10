use std::collections::HashMap;

use ilold_session_core::exploration::assign_operator::AssignOperator;
use ilold_session_core::exploration::session::{
    ExplorationSession, ExplorationStep, MutationScope, StateMutation, TraceConfig,
};
use ilold_session_core::journal::types::JournalEntry;
use ilold_session_core::runtime_trace::{
    AccountDiff, InnerInstruction as TraceInnerInstruction, RuntimeTrace,
};
use litesvm::types::TransactionMetadata;
use serde_json::Value;
use solana_account::{Account, ReadableAccount};
use solana_address::Address;
use solana_keypair::Keypair;

use crate::error::SolanaError;
use crate::execute::{build_instruction, build_transaction, VmHost};
use crate::model::{InstructionDef, ProgramDef};

/// Outcome of executing a Call against the VM.
///
/// `step_index = Some(N)` means the call succeeded and was appended to
/// `session.steps[N]`. `step_index = None` means the VM rejected the call
/// (Anchor constraint, custom `require!`, runtime panic) — we deliberately
/// do NOT push a step in that case so the scenario timeline only contains
/// runs that actually mutated state. Mirrors how Solidity's `c <fn>` only
/// records valid entry points; the auditor still gets the full trace via
/// the `trace` field for inspection.
pub struct StepOutcome {
    pub step_index: Option<usize>,
    pub trace: RuntimeTrace,
}

#[allow(clippy::too_many_arguments)]
pub fn add_solana_step(
    session: &mut ExplorationSession,
    program: &ProgramDef,
    ix: &InstructionDef,
    vm: &mut VmHost,
    args: Value,
    accounts: HashMap<String, Address>,
    extra_signers: &[&Keypair],
    timestamp: &str,
    call_payload: Option<Value>,
) -> Result<StepOutcome, SolanaError> {
    let step_index = session.steps.len();
    let types = &program.types;

    let instruction =
        build_instruction(program.program_id, ix, args, accounts, types)?;

    let pre_state: Vec<(Address, Option<Account>)> = instruction
        .accounts
        .iter()
        .map(|m| (m.pubkey, vm.svm().get_account(&m.pubkey)))
        .collect();

    let blockhash = vm.svm().latest_blockhash();
    let tx = build_transaction(instruction.clone(), vm.payer(), extra_signers, blockhash)?;
    let account_keys: Vec<Address> = tx.message.static_account_keys().to_vec();
    let result = vm.svm_mut().send_transaction(tx);
    // LiteSVM does NOT rotate the blockhash automatically. Two Calls in the same
    // session would collide (BlockhashNotFound) and the second silently fails
    // with cu=0 / no state mutation. Expire after every send so the next Call
    // gets a fresh blockhash.
    vm.svm_mut().expire_blockhash();

    let (runtime_trace, mutations) = match result {
        Ok(meta) => {
            let post_state: Vec<(Address, Option<Account>)> = pre_state
                .iter()
                .map(|(addr, _)| (*addr, vm.svm().get_account(addr)))
                .collect();

            let diffs = compute_diffs(&pre_state, &post_state, ix);
            let mutations = diffs_to_mutations(&diffs, step_index);
            let inner = project_inner_instructions(&meta, &account_keys);
            let return_data = if meta.return_data.data.is_empty() {
                None
            } else {
                Some(meta.return_data.data)
            };
            (
                RuntimeTrace {
                    logs: meta.logs,
                    compute_units: meta.compute_units_consumed,
                    inner_instructions: inner,
                    account_diffs: diffs,
                    return_data,
                    error: None,
                },
                mutations,
            )
        }
        Err(failed) => {
            let inner = project_inner_instructions(&failed.meta, &account_keys);
            (
                RuntimeTrace {
                    logs: failed.meta.logs,
                    compute_units: failed.meta.compute_units_consumed,
                    inner_instructions: inner,
                    account_diffs: vec![],
                    return_data: None,
                    error: Some(format!("{:?}", failed.err)),
                },
                vec![],
            )
        }
    };

    // Failed Calls never reach the timeline: the auditor wanted to try the
    // attack, the VM blocked it, and the canonical model is "session steps
    // are real successful transactions". The full trace is still returned
    // so the CLI can print logs / CU / error.
    if runtime_trace.error.is_some() {
        return Ok(StepOutcome { step_index: None, trace: runtime_trace });
    }

    let trace_value = serde_json::to_value(&runtime_trace).ok();

    session.steps.push(ExplorationStep {
        function: ix.name.clone(),
        mutations,
        flow_tree: None,
        trace_config: TraceConfig::default(),
        runtime_trace: trace_value,
        call_payload,
    });

    session
        .journal
        .record(JournalEntry::SequenceExplored {
            steps: session.steps.iter().map(|s| s.function.clone()).collect(),
            timestamp: timestamp.into(),
        });

    Ok(StepOutcome {
        step_index: Some(step_index),
        trace: runtime_trace,
    })
}


// Maps the LiteSVM `inner_instructions` (Vec<Vec<InnerInstruction>>, one outer
// entry per top-level ix) into the trace shape consumed by the overlay. The
// `program_id_index` is into `tx.message.static_account_keys`, captured before
// `send_transaction` consumed the tx.
fn project_inner_instructions(
    meta: &TransactionMetadata,
    account_keys: &[Address],
) -> Vec<TraceInnerInstruction> {
    let mut out = Vec::new();
    for level in &meta.inner_instructions {
        for ii in level {
            let program = account_keys
                .get(ii.instruction.program_id_index as usize)
                .map(|k| k.to_string())
                .unwrap_or_else(|| format!("idx:{}", ii.instruction.program_id_index));
            // Anchor instruction discriminators are the first 8 bytes; the
            // first byte (or its bs58 head) is enough as a stable, legible
            // disambiguator for the overlay aggregation key. Empty payload
            // gets a placeholder so cpi_edges still aggregates by program.
            let instruction = if ii.instruction.data.is_empty() {
                "ix".to_string()
            } else {
                let take = ii.instruction.data.len().min(8);
                bs58::encode(&ii.instruction.data[..take]).into_string()
            };
            out.push(TraceInnerInstruction {
                program,
                instruction,
                depth: ii.stack_height as u32,
            });
        }
    }
    out
}

fn compute_diffs(
    pre: &[(Address, Option<Account>)],
    post: &[(Address, Option<Account>)],
    ix: &InstructionDef,
) -> Vec<AccountDiff> {
    pre.iter()
        .zip(post.iter())
        .enumerate()
        .filter_map(|(idx, ((addr, before), (_, after)))| {
            let lamports_before = before.as_ref().map(|a| a.lamports()).unwrap_or(0);
            let lamports_after = after.as_ref().map(|a| a.lamports()).unwrap_or(0);
            let data_before = before.as_ref().map(|a| a.data().to_vec());
            let data_after = after.as_ref().map(|a| a.data().to_vec());
            let owner_before = before.as_ref().map(|a| a.owner);
            let owner_after = after.as_ref().map(|a| a.owner);

            let lamports_changed = lamports_before != lamports_after;
            let data_changed = data_before != data_after;
            let owner_changed = owner_before != owner_after;

            if !lamports_changed && !data_changed && !owner_changed {
                return None;
            }

            Some(AccountDiff {
                address: addr.to_string(),
                name: ix.accounts.get(idx).map(|s| s.name.clone()),
                before: data_before,
                after: data_after,
                lamports_delta: (lamports_after as i128) - (lamports_before as i128),
                owner_changed,
                decoded_before: None,
                decoded_after: None,
            })
        })
        .collect()
}

fn diffs_to_mutations(diffs: &[AccountDiff], step_index: usize) -> Vec<StateMutation> {
    let mut out = Vec::new();
    for d in diffs {
        let label = d.name.clone().unwrap_or_else(|| d.address.clone());
        if d.lamports_delta != 0 {
            out.push(StateMutation {
                variable: format!("{label}.lamports"),
                operator: AssignOperator::Assign,
                value_expr: d.lamports_delta.to_string(),
                step_index,
                via: None,
                flow_step_id: None,
                scope: MutationScope::State,
            });
        }
        if d.before != d.after {
            out.push(StateMutation {
                variable: format!("{label}.data"),
                operator: AssignOperator::Assign,
                value_expr: d
                    .after
                    .as_ref()
                    .map(|b| format!("{} bytes", b.len()))
                    .unwrap_or_else(|| "<closed>".into()),
                step_index,
                via: None,
                flow_step_id: None,
                scope: MutationScope::State,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AccountSpec;
    use solana_keypair::Keypair;
    use solana_signer::Signer;

    fn ix_with_accounts(names: &[&str]) -> InstructionDef {
        InstructionDef {
            name: "test".into(),
            discriminator: [0u8; 8],
            args: vec![],
            accounts: names
                .iter()
                .map(|n| AccountSpec {
                    path: n.to_string(),
                    name: n.to_string(),
                    writable: true,
                    signer: false,
                    optional: false,
                    address: None,
                    pda: None,
                    relations: vec![],
                })
                .collect(),
            returns: None,
        }
    }

    fn account_with(lamports: u64, data: Vec<u8>) -> Account {
        Account {
            lamports,
            data,
            owner: Address::default(),
            executable: false,
            rent_epoch: 0,
        }
    }

    #[test]
    fn diff_skips_unchanged_accounts() {
        let addr = Keypair::new().pubkey();
        let acc = account_with(100, vec![1, 2, 3]);
        let pre = vec![(addr, Some(acc.clone()))];
        let post = vec![(addr, Some(acc))];
        let ix = ix_with_accounts(&["a"]);

        let diffs = compute_diffs(&pre, &post, &ix);
        assert!(diffs.is_empty());
    }

    #[test]
    fn diff_detects_lamports_and_data_changes() {
        let addr = Keypair::new().pubkey();
        let pre = vec![(addr, Some(account_with(100, vec![1])))];
        let post = vec![(addr, Some(account_with(150, vec![1, 2, 3])))];
        let ix = ix_with_accounts(&["counter"]);

        let diffs = compute_diffs(&pre, &post, &ix);
        assert_eq!(diffs.len(), 1);
        let d = &diffs[0];
        assert_eq!(d.lamports_delta, 50);
        assert_eq!(d.before, Some(vec![1]));
        assert_eq!(d.after, Some(vec![1, 2, 3]));
        assert_eq!(d.name.as_deref(), Some("counter"));
    }

    #[test]
    fn diff_handles_account_creation() {
        let addr = Keypair::new().pubkey();
        let pre = vec![(addr, None)];
        let post = vec![(addr, Some(account_with(890, vec![0; 8])))];
        let ix = ix_with_accounts(&["new_acc"]);

        let diffs = compute_diffs(&pre, &post, &ix);
        assert_eq!(diffs.len(), 1);
        let d = &diffs[0];
        assert_eq!(d.lamports_delta, 890);
        assert_eq!(d.before, None);
        assert_eq!(d.after, Some(vec![0; 8]));
    }

    #[test]
    fn mutations_emit_lamports_and_data_entries() {
        let diffs = vec![AccountDiff {
            address: "abc".into(),
            name: Some("counter".into()),
            before: Some(vec![0]),
            after: Some(vec![42]),
            lamports_delta: 1_000,
            owner_changed: false,
            decoded_before: None,
            decoded_after: None,
        }];

        let muts = diffs_to_mutations(&diffs, 3);
        assert_eq!(muts.len(), 2);
        assert_eq!(muts[0].variable, "counter.lamports");
        assert_eq!(muts[0].value_expr, "1000");
        assert_eq!(muts[0].step_index, 3);
        assert_eq!(muts[1].variable, "counter.data");
        assert_eq!(muts[1].value_expr, "1 bytes");
    }
}
