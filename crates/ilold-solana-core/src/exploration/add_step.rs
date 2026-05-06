use std::collections::HashMap;

use ilold_session_core::exploration::assign_operator::AssignOperator;
use ilold_session_core::exploration::session::{
    ExplorationSession, ExplorationStep, MutationScope, StateMutation, TraceConfig,
};
use ilold_session_core::journal::types::JournalEntry;
use ilold_session_core::runtime_trace::{AccountDiff, RuntimeTrace};
use serde_json::Value;
use solana_account::{Account, ReadableAccount};
use solana_address::Address;

use crate::error::SolanaError;
use crate::execute::{build_instruction, build_transaction, VmHost};
use crate::model::{InstructionDef, ProgramDef};

#[allow(clippy::too_many_arguments)]
pub fn add_solana_step<'a>(
    session: &'a mut ExplorationSession,
    program: &ProgramDef,
    ix: &InstructionDef,
    vm: &mut VmHost,
    args: Value,
    accounts: HashMap<String, Address>,
    timestamp: &str,
) -> Result<&'a ExplorationStep, SolanaError> {
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
    let tx = build_transaction(instruction.clone(), vm.payer(), blockhash)?;
    let result = vm.svm_mut().send_transaction(tx);

    let (runtime_trace, mutations) = match result {
        Ok(meta) => {
            let post_state: Vec<(Address, Option<Account>)> = pre_state
                .iter()
                .map(|(addr, _)| (*addr, vm.svm().get_account(addr)))
                .collect();

            let diffs = compute_diffs(&pre_state, &post_state, ix);
            let mutations = diffs_to_mutations(&diffs, step_index);
            let return_data = if meta.return_data.data.is_empty() {
                None
            } else {
                Some(meta.return_data.data)
            };
            (
                RuntimeTrace {
                    logs: meta.logs,
                    compute_units: meta.compute_units_consumed,
                    inner_instructions: vec![],
                    account_diffs: diffs,
                    return_data,
                    error: None,
                },
                mutations,
            )
        }
        Err(failed) => (
            RuntimeTrace {
                logs: failed.meta.logs,
                compute_units: failed.meta.compute_units_consumed,
                inner_instructions: vec![],
                account_diffs: vec![],
                return_data: None,
                error: Some(format!("{:?}", failed.err)),
            },
            vec![],
        ),
    };

    let trace_value = serde_json::to_value(&runtime_trace).ok();

    session.steps.push(ExplorationStep {
        function: ix.name.clone(),
        mutations,
        flow_tree: None,
        trace_config: TraceConfig::default(),
        runtime_trace: trace_value,
    });

    session
        .journal
        .record(JournalEntry::SequenceExplored {
            steps: session.steps.iter().map(|s| s.function.clone()).collect(),
            timestamp: timestamp.into(),
        });

    Ok(session.steps.last().unwrap())
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
