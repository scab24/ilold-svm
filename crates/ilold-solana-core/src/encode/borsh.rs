use anchor_lang_idl::types::{
    IdlArrayLen, IdlDefinedFields, IdlEnumVariant, IdlType, IdlTypeDef, IdlTypeDefTy,
};
use borsh::BorshSerialize;
use serde_json::Value;

use crate::error::SolanaError;
use crate::model::InstructionDef;

pub fn encode_value(
    value: &Value,
    ty: &IdlType,
    types: &[IdlTypeDef],
) -> Result<Vec<u8>, SolanaError> {
    let mut buf = Vec::new();
    write_value(&mut buf, value, ty, types)?;
    Ok(buf)
}

pub fn encode_ix_data(
    ix: &InstructionDef,
    args: &Value,
    types: &[IdlTypeDef],
) -> Result<Vec<u8>, SolanaError> {
    let mut buf = Vec::with_capacity(8);
    buf.extend_from_slice(&ix.discriminator);
    let obj = args
        .as_object()
        .ok_or_else(|| SolanaError::EncodeTypeMismatch {
            expected: "object with named args".into(),
            got: format!("{args}"),
        })?;
    for arg in &ix.args {
        let v = obj.get(&arg.name).ok_or_else(|| SolanaError::EncodeFailed(format!(
            "missing instruction arg '{}'",
            arg.name
        )))?;
        write_value(&mut buf, v, &arg.ty, types)?;
    }
    Ok(buf)
}

fn write_value(
    buf: &mut Vec<u8>,
    value: &Value,
    ty: &IdlType,
    types: &[IdlTypeDef],
) -> Result<(), SolanaError> {
    match ty {
        IdlType::Bool => write_primitive::<bool>(buf, value.as_bool().ok_or_else(|| mismatch("bool", value))?),
        IdlType::U8 => write_primitive::<u8>(
            buf,
            u8::try_from(value.as_u64().ok_or_else(|| mismatch("u8", value))?)
                .map_err(|_| mismatch("u8", value))?,
        ),
        IdlType::I8 => write_primitive::<i8>(
            buf,
            i8::try_from(value.as_i64().ok_or_else(|| mismatch("i8", value))?)
                .map_err(|_| mismatch("i8", value))?,
        ),
        IdlType::U16 => write_primitive::<u16>(
            buf,
            u16::try_from(value.as_u64().ok_or_else(|| mismatch("u16", value))?)
                .map_err(|_| mismatch("u16", value))?,
        ),
        IdlType::I16 => write_primitive::<i16>(
            buf,
            i16::try_from(value.as_i64().ok_or_else(|| mismatch("i16", value))?)
                .map_err(|_| mismatch("i16", value))?,
        ),
        IdlType::U32 => write_primitive::<u32>(
            buf,
            u32::try_from(value.as_u64().ok_or_else(|| mismatch("u32", value))?)
                .map_err(|_| mismatch("u32", value))?,
        ),
        IdlType::I32 => write_primitive::<i32>(
            buf,
            i32::try_from(value.as_i64().ok_or_else(|| mismatch("i32", value))?)
                .map_err(|_| mismatch("i32", value))?,
        ),
        IdlType::F32 => write_primitive::<f32>(buf, value.as_f64().ok_or_else(|| mismatch("f32", value))? as f32),
        IdlType::U64 => write_primitive::<u64>(buf, value.as_u64().ok_or_else(|| mismatch("u64", value))?),
        IdlType::I64 => write_primitive::<i64>(buf, value.as_i64().ok_or_else(|| mismatch("i64", value))?),
        IdlType::F64 => write_primitive::<f64>(buf, value.as_f64().ok_or_else(|| mismatch("f64", value))?),
        IdlType::U128 => {
            let s = value.as_str().ok_or_else(|| mismatch("u128 decimal string", value))?;
            let v: u128 = s.parse().map_err(|_| mismatch("u128 decimal string", value))?;
            write_primitive::<u128>(buf, v)
        }
        IdlType::I128 => {
            let s = value.as_str().ok_or_else(|| mismatch("i128 decimal string", value))?;
            let v: i128 = s.parse().map_err(|_| mismatch("i128 decimal string", value))?;
            write_primitive::<i128>(buf, v)
        }
        IdlType::U256 | IdlType::I256 => {
            let s = value.as_str().ok_or_else(|| mismatch("u/i256 hex string", value))?;
            let stripped = s.strip_prefix("0x").unwrap_or(s);
            let bytes = decode_hex(stripped).ok_or_else(|| mismatch("u/i256 hex string", value))?;
            if bytes.len() != 32 {
                return Err(mismatch("u/i256 32-byte hex", value));
            }
            buf.extend_from_slice(&bytes);
            Ok(())
        }
        IdlType::String => {
            let s = value.as_str().ok_or_else(|| mismatch("string", value))?;
            write_primitive::<String>(buf, s.to_string())
        }
        IdlType::Bytes => {
            let s = value.as_str().ok_or_else(|| mismatch("bytes hex string", value))?;
            let bytes = decode_hex(s).ok_or_else(|| mismatch("bytes hex string", value))?;
            write_primitive::<Vec<u8>>(buf, bytes)
        }
        IdlType::Pubkey => {
            let s = value.as_str().ok_or_else(|| mismatch("pubkey base58", value))?;
            let bytes = bs58::decode(s)
                .into_vec()
                .map_err(|_| mismatch("pubkey base58", value))?;
            if bytes.len() != 32 {
                return Err(mismatch("pubkey 32 bytes", value));
            }
            buf.extend_from_slice(&bytes);
            Ok(())
        }
        IdlType::Option(inner) => {
            if value.is_null() {
                buf.push(0);
                Ok(())
            } else {
                buf.push(1);
                write_value(buf, value, inner, types)
            }
        }
        IdlType::Vec(inner) => {
            let arr = value.as_array().ok_or_else(|| mismatch("array", value))?;
            let len = u32::try_from(arr.len())
                .map_err(|_| SolanaError::EncodeFailed("vec length exceeds u32".into()))?;
            write_primitive::<u32>(buf, len)?;
            for item in arr {
                write_value(buf, item, inner, types)?;
            }
            Ok(())
        }
        IdlType::Array(inner, IdlArrayLen::Value(n)) => {
            let arr = value.as_array().ok_or_else(|| mismatch("array", value))?;
            if arr.len() != *n {
                return Err(SolanaError::EncodeFailed(format!(
                    "expected array of {} elements, got {}",
                    n,
                    arr.len()
                )));
            }
            for item in arr {
                write_value(buf, item, inner, types)?;
            }
            Ok(())
        }
        IdlType::Array(_, IdlArrayLen::Generic(name)) => {
            Err(SolanaError::UnsupportedGeneric(format!("array length generic '{name}'")))
        }
        IdlType::Defined { name, .. } => {
            let def = types
                .iter()
                .find(|t| &t.name == name)
                .ok_or_else(|| SolanaError::UnknownType(name.clone()))?;
            write_typedef(buf, value, def, types)
        }
        IdlType::Generic(name) => Err(SolanaError::UnsupportedGeneric(name.clone())),
        _ => Err(SolanaError::EncodeFailed("unsupported IdlType variant".into())),
    }
}

fn write_typedef(
    buf: &mut Vec<u8>,
    value: &Value,
    def: &IdlTypeDef,
    types: &[IdlTypeDef],
) -> Result<(), SolanaError> {
    match &def.ty {
        IdlTypeDefTy::Struct { fields } => write_defined_fields(buf, value, fields.as_ref(), types),
        IdlTypeDefTy::Enum { variants } => write_enum(buf, value, variants, types),
        IdlTypeDefTy::Type { alias } => write_value(buf, value, alias, types),
    }
}

fn write_defined_fields(
    buf: &mut Vec<u8>,
    value: &Value,
    fields: Option<&IdlDefinedFields>,
    types: &[IdlTypeDef],
) -> Result<(), SolanaError> {
    match fields {
        None => Ok(()),
        Some(IdlDefinedFields::Named(items)) => {
            let obj = value
                .as_object()
                .ok_or_else(|| mismatch("object with named fields", value))?;
            for f in items {
                let v = obj.get(&f.name).ok_or_else(|| {
                    SolanaError::EncodeFailed(format!("missing struct field '{}'", f.name))
                })?;
                write_value(buf, v, &f.ty, types)?;
            }
            Ok(())
        }
        Some(IdlDefinedFields::Tuple(items)) => {
            let arr = value
                .as_array()
                .ok_or_else(|| mismatch("array for tuple struct", value))?;
            if arr.len() != items.len() {
                return Err(SolanaError::EncodeFailed(format!(
                    "tuple expected {} elements, got {}",
                    items.len(),
                    arr.len()
                )));
            }
            for (v, ty) in arr.iter().zip(items.iter()) {
                write_value(buf, v, ty, types)?;
            }
            Ok(())
        }
    }
}

fn write_enum(
    buf: &mut Vec<u8>,
    value: &Value,
    variants: &[IdlEnumVariant],
    types: &[IdlTypeDef],
) -> Result<(), SolanaError> {
    let obj = value
        .as_object()
        .ok_or_else(|| mismatch("enum object {kind, value?}", value))?;
    let kind = obj
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| mismatch("enum kind string", value))?;
    let (idx, variant) = variants
        .iter()
        .enumerate()
        .find(|(_, v)| v.name == kind)
        .ok_or_else(|| SolanaError::EncodeFailed(format!("unknown enum variant '{kind}'")))?;
    let tag = u8::try_from(idx).map_err(|_| SolanaError::EncodeFailed("enum tag > 255".into()))?;
    buf.push(tag);

    match (&variant.fields, obj.get("value")) {
        (None, _) => Ok(()),
        (Some(fields), Some(payload)) => {
            write_defined_fields(buf, payload, Some(fields), types)
        }
        (Some(_), None) => Err(SolanaError::EncodeFailed(format!(
            "enum variant '{kind}' expects payload but got none"
        ))),
    }
}

fn write_primitive<T: BorshSerialize>(buf: &mut Vec<u8>, v: T) -> Result<(), SolanaError> {
    v.serialize(buf)
        .map_err(|e| SolanaError::EncodeFailed(e.to_string()))
}

fn mismatch(expected: &str, got: &Value) -> SolanaError {
    SolanaError::EncodeTypeMismatch {
        expected: expected.to_string(),
        got: format!("{got}"),
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

