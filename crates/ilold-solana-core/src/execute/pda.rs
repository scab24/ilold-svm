use std::collections::HashMap;

use anchor_lang_idl::types::IdlType;
use serde_json::Value;
use solana_address::Address;

use crate::error::SolanaError;
use crate::model::{PdaSpec, SeedSpec};

pub fn derive_pda(
    spec: &PdaSpec,
    program: Address,
    args: &Value,
    accounts: &HashMap<String, Address>,
) -> Result<(Address, u8), SolanaError> {
    let mut seed_bytes: Vec<Vec<u8>> = Vec::with_capacity(spec.seeds.len());
    for seed in &spec.seeds {
        seed_bytes.push(resolve_seed(seed, args, accounts)?);
    }

    let program_id = match &spec.program {
        None => program,
        Some(SeedSpec::Const { value }) => Address::try_from(value.as_slice())
            .map_err(|_| SolanaError::InvalidProgramId(format!("{:02x?}", value)))?,
        Some(SeedSpec::Account { path }) => *accounts
            .get(path)
            .ok_or_else(|| SolanaError::SeedArgUnresolved { path: path.clone() })?,
        Some(SeedSpec::Arg { path, .. }) => {
            return Err(SolanaError::PdaProgramArgUnsupported { path: path.clone() });
        }
    };

    let refs: Vec<&[u8]> = seed_bytes.iter().map(Vec::as_slice).collect();
    let (pda, bump) = Address::find_program_address(&refs, &program_id);
    Ok((pda, bump))
}

fn resolve_seed(
    seed: &SeedSpec,
    args: &Value,
    accounts: &HashMap<String, Address>,
) -> Result<Vec<u8>, SolanaError> {
    match seed {
        SeedSpec::Const { value } => Ok(value.clone()),
        SeedSpec::Account { path } => accounts
            .get(path)
            .map(|pk| pk.to_bytes().to_vec())
            .ok_or_else(|| SolanaError::SeedArgUnresolved { path: path.clone() }),
        SeedSpec::Arg { path, ty } => {
            let value = args
                .pointer(&format!("/{}", path.replace('.', "/")))
                .ok_or_else(|| SolanaError::SeedArgUnresolved { path: path.clone() })?;
            encode_arg_seed(value, ty, path)
        }
    }
}

fn encode_arg_seed(value: &Value, ty: &IdlType, path: &str) -> Result<Vec<u8>, SolanaError> {
    let mismatch = |expected: &str| SolanaError::SeedTypeMismatch {
        path: path.to_string(),
        expected: expected.to_string(),
        got: format!("{value}"),
    };
    match (ty, value) {
        (IdlType::String, Value::String(s)) => Ok(s.as_bytes().to_vec()),
        (IdlType::Bytes, Value::String(hex)) => decode_hex(hex).ok_or_else(|| mismatch("bytes hex")),
        (IdlType::Pubkey, Value::String(b58)) => bs58::decode(b58)
            .into_vec()
            .map_err(|_| mismatch("pubkey base58")),
        (IdlType::U8, Value::Number(n)) => {
            let v = u8::try_from(n.as_u64().ok_or_else(|| mismatch("u8"))?)
                .map_err(|_| mismatch("u8"))?;
            Ok(v.to_le_bytes().to_vec())
        }
        (IdlType::U16, Value::Number(n)) => {
            let v = u16::try_from(n.as_u64().ok_or_else(|| mismatch("u16"))?)
                .map_err(|_| mismatch("u16"))?;
            Ok(v.to_le_bytes().to_vec())
        }
        (IdlType::U32, Value::Number(n)) => {
            let v = u32::try_from(n.as_u64().ok_or_else(|| mismatch("u32"))?)
                .map_err(|_| mismatch("u32"))?;
            Ok(v.to_le_bytes().to_vec())
        }
        (IdlType::U64, Value::Number(n)) => Ok(n
            .as_u64()
            .ok_or_else(|| mismatch("u64"))?
            .to_le_bytes()
            .to_vec()),
        (IdlType::I8, Value::Number(n)) => {
            let v = i8::try_from(n.as_i64().ok_or_else(|| mismatch("i8"))?)
                .map_err(|_| mismatch("i8"))?;
            Ok(v.to_le_bytes().to_vec())
        }
        (IdlType::I16, Value::Number(n)) => {
            let v = i16::try_from(n.as_i64().ok_or_else(|| mismatch("i16"))?)
                .map_err(|_| mismatch("i16"))?;
            Ok(v.to_le_bytes().to_vec())
        }
        (IdlType::I32, Value::Number(n)) => {
            let v = i32::try_from(n.as_i64().ok_or_else(|| mismatch("i32"))?)
                .map_err(|_| mismatch("i32"))?;
            Ok(v.to_le_bytes().to_vec())
        }
        (IdlType::I64, Value::Number(n)) => Ok(n
            .as_i64()
            .ok_or_else(|| mismatch("i64"))?
            .to_le_bytes()
            .to_vec()),
        _ => Err(SolanaError::SeedTypeMismatch {
            path: path.to_string(),
            expected: format!("{ty:?}"),
            got: format!("{value}"),
        }),
    }
}

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    for i in (0..s.len()).step_by(2) {
        out.push(u8::from_str_radix(&s[i..i + 2], 16).ok()?);
    }
    Some(out)
}
