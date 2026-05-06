use std::collections::HashMap;

use anchor_lang_idl::types::IdlTypeDef;
use serde_json::Value;
use solana_address::Address;
use solana_hash::Hash;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

use crate::encode::encode_ix_data;
use crate::error::SolanaError;
use crate::execute::pda::derive_pda;
use crate::model::InstructionDef;

pub fn build_instruction(
    program_id: Address,
    ix: &InstructionDef,
    mut args: Value,
    accounts: HashMap<String, Address>,
    types: &[IdlTypeDef],
) -> Result<Instruction, SolanaError> {
    let mut resolved = accounts;

    for spec in &ix.accounts {
        if resolved.contains_key(&spec.path) || resolved.contains_key(&spec.name) {
            continue;
        }
        if let Some(pda_spec) = &spec.pda {
            let (pda, bump) = derive_pda(pda_spec, program_id, &args, &resolved)?;
            resolved.insert(spec.path.clone(), pda);
            if let Some(arg_name) = &pda_spec.bump_arg {
                if let Some(obj) = args.as_object_mut() {
                    obj.insert(arg_name.clone(), Value::from(bump));
                }
            }
        } else if let Some(addr) = spec.address {
            resolved.insert(spec.path.clone(), addr);
        } else {
            return Err(SolanaError::AccountNotProvided {
                path: spec.path.clone(),
            });
        }
    }

    let metas: Vec<AccountMeta> = ix
        .accounts
        .iter()
        .map(|spec| {
            let addr = resolved
                .get(&spec.path)
                .or_else(|| resolved.get(&spec.name))
                .copied()
                .ok_or_else(|| SolanaError::AccountNotProvided {
                    path: spec.path.clone(),
                })?;
            Ok(if spec.writable {
                AccountMeta::new(addr, spec.signer)
            } else {
                AccountMeta::new_readonly(addr, spec.signer)
            })
        })
        .collect::<Result<Vec<_>, SolanaError>>()?;

    let data = encode_ix_data(ix, &args, types)?;

    Ok(Instruction {
        program_id,
        accounts: metas,
        data,
    })
}

pub fn build_transaction(
    ix: Instruction,
    payer: &Keypair,
    blockhash: Hash,
) -> Result<VersionedTransaction, SolanaError> {
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer])
        .map_err(|e| SolanaError::EncodeFailed(format!("transaction sign: {e:?}")))
}
