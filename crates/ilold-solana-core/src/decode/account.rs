use anchor_lang_idl::types::{Idl, IdlTypeDefTy};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::decode::borsh::decode_defined_fields;
use crate::error::SolanaError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodedAccount {
    pub type_name: String,
    pub value: Value,
}

pub fn decode_account(data: &[u8], idl: &Idl) -> Result<DecodedAccount, SolanaError> {
    if data.len() < 8 {
        return Err(SolanaError::DecodeFailed(format!(
            "account data is {} bytes, need at least 8 for discriminator",
            data.len()
        )));
    }
    let (disc, rest) = data.split_at(8);
    let account = idl
        .accounts
        .iter()
        .find(|a| a.discriminator == disc)
        .ok_or_else(|| SolanaError::UnknownDiscriminator {
            hex: disc.iter().map(|b| format!("{:02x}", b)).collect(),
        })?;
    let type_def = idl
        .types
        .iter()
        .find(|t| t.name == account.name)
        .ok_or_else(|| SolanaError::UnknownType(account.name.clone()))?;
    let mut cursor = rest;
    let value = match &type_def.ty {
        IdlTypeDefTy::Struct { fields } => {
            decode_defined_fields(&mut cursor, fields.as_ref(), &idl.types)?
        }
        _ => {
            return Err(SolanaError::DecodeFailed(format!(
                "account '{}' is not a struct typedef",
                account.name
            )));
        }
    };
    Ok(DecodedAccount {
        type_name: account.name.clone(),
        value,
    })
}
