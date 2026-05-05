use anchor_lang_idl::types::IdlTypeDef;
use serde_json::{Map, Value};

use crate::decode::borsh::decode_value;
use crate::error::SolanaError;
use crate::model::InstructionDef;

pub fn decode_ix_data(
    data: &[u8],
    ix: &InstructionDef,
    types: &[IdlTypeDef],
) -> Result<Value, SolanaError> {
    if data.len() < 8 {
        return Err(SolanaError::DecodeFailed(format!(
            "instruction data is {} bytes, need at least 8 for discriminator",
            data.len()
        )));
    }
    let (disc, mut rest) = data.split_at(8);
    if disc != ix.discriminator {
        return Err(SolanaError::DecodeFailed(format!(
            "instruction discriminator does not match '{}'",
            ix.name
        )));
    }
    let mut obj = Map::with_capacity(ix.args.len());
    for arg in &ix.args {
        obj.insert(arg.name.clone(), decode_value(&mut rest, &arg.ty, types)?);
    }
    Ok(Value::Object(obj))
}
