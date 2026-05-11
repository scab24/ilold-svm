use std::io::Read;

use anchor_lang_idl::types::{
    IdlArrayLen, IdlDefinedFields, IdlEnumVariant, IdlType, IdlTypeDef, IdlTypeDefTy,
};
use borsh::BorshDeserialize;
use serde_json::{Map, Value};

use crate::error::SolanaError;

pub fn decode_value(
    reader: &mut &[u8],
    ty: &IdlType,
    types: &[IdlTypeDef],
) -> Result<Value, SolanaError> {
    Ok(match ty {
        IdlType::Bool => Value::Bool(read::<bool>(reader)?),
        IdlType::U8 => Value::Number(read::<u8>(reader)?.into()),
        IdlType::I8 => Value::Number(read::<i8>(reader)?.into()),
        IdlType::U16 => Value::Number(read::<u16>(reader)?.into()),
        IdlType::I16 => Value::Number(read::<i16>(reader)?.into()),
        IdlType::U32 => Value::Number(read::<u32>(reader)?.into()),
        IdlType::I32 => Value::Number(read::<i32>(reader)?.into()),
        IdlType::F32 => json_number_or_string(read::<f32>(reader)? as f64),
        IdlType::U64 => Value::Number(read::<u64>(reader)?.into()),
        IdlType::I64 => Value::Number(read::<i64>(reader)?.into()),
        IdlType::F64 => json_number_or_string(read::<f64>(reader)?),
        IdlType::U128 => Value::String(read::<u128>(reader)?.to_string()),
        IdlType::I128 => Value::String(read::<i128>(reader)?.to_string()),
        IdlType::U256 | IdlType::I256 => {
            let mut buf = [0u8; 32];
            read_exact(reader, &mut buf)?;
            Value::String(format!("0x{}", hex_le(&buf)))
        }
        IdlType::String => Value::String(read::<String>(reader)?),
        IdlType::Bytes => Value::String(hex_le(&read::<Vec<u8>>(reader)?)),
        IdlType::Pubkey => {
            let mut buf = [0u8; 32];
            read_exact(reader, &mut buf)?;
            Value::String(bs58::encode(buf).into_string())
        }
        IdlType::Option(inner) => match read::<u8>(reader)? {
            0 => Value::Null,
            1 => decode_value(reader, inner, types)?,
            tag => {
                return Err(SolanaError::DecodeFailed(format!(
                    "invalid Option tag {tag}"
                )));
            }
        },
        IdlType::Vec(inner) => {
            let len = read::<u32>(reader)? as usize;
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                arr.push(decode_value(reader, inner, types)?);
            }
            Value::Array(arr)
        }
        IdlType::Array(inner, IdlArrayLen::Value(n)) => {
            let mut arr = Vec::with_capacity(*n);
            for _ in 0..*n {
                arr.push(decode_value(reader, inner, types)?);
            }
            Value::Array(arr)
        }
        IdlType::Array(_, IdlArrayLen::Generic(name)) => {
            return Err(SolanaError::UnsupportedGeneric(format!(
                "array length generic '{name}'"
            )));
        }
        IdlType::Defined { name, .. } => {
            let def = types
                .iter()
                .find(|t| &t.name == name)
                .ok_or_else(|| SolanaError::UnknownType(name.clone()))?;
            decode_typedef(reader, def, types)?
        }
        IdlType::Generic(name) => {
            return Err(SolanaError::UnsupportedGeneric(name.clone()));
        }
        _ => return Err(SolanaError::DecodeFailed("unsupported IdlType variant".into())),
    })
}

pub(crate) fn decode_typedef(
    reader: &mut &[u8],
    def: &IdlTypeDef,
    types: &[IdlTypeDef],
) -> Result<Value, SolanaError> {
    match &def.ty {
        IdlTypeDefTy::Struct { fields } => decode_defined_fields(reader, fields.as_ref(), types),
        IdlTypeDefTy::Enum { variants } => decode_enum(reader, variants, types),
        IdlTypeDefTy::Type { alias } => decode_value(reader, alias, types),
    }
}

pub(crate) fn decode_defined_fields(
    reader: &mut &[u8],
    fields: Option<&IdlDefinedFields>,
    types: &[IdlTypeDef],
) -> Result<Value, SolanaError> {
    match fields {
        None => Ok(Value::Object(Map::new())),
        Some(IdlDefinedFields::Named(items)) => {
            let mut obj = Map::with_capacity(items.len());
            for f in items {
                obj.insert(f.name.clone(), decode_value(reader, &f.ty, types)?);
            }
            Ok(Value::Object(obj))
        }
        Some(IdlDefinedFields::Tuple(items)) => {
            let mut arr = Vec::with_capacity(items.len());
            for ty in items {
                arr.push(decode_value(reader, ty, types)?);
            }
            Ok(Value::Array(arr))
        }
    }
}

fn decode_enum(
    reader: &mut &[u8],
    variants: &[IdlEnumVariant],
    types: &[IdlTypeDef],
) -> Result<Value, SolanaError> {
    let tag = read::<u8>(reader)? as usize;
    let variant = variants.get(tag).ok_or_else(|| {
        SolanaError::DecodeFailed(format!(
            "enum tag {tag} out of range (have {} variants)",
            variants.len()
        ))
    })?;
    let payload = decode_defined_fields(reader, variant.fields.as_ref(), types)?;
    let payload_empty = payload
        .as_object()
        .map(|o| o.is_empty())
        .or_else(|| payload.as_array().map(|a| a.is_empty()))
        .unwrap_or(false);
    let mut obj = Map::new();
    obj.insert("kind".into(), Value::String(variant.name.clone()));
    if !payload_empty {
        obj.insert("value".into(), payload);
    }
    Ok(Value::Object(obj))
}

fn read<T: BorshDeserialize>(reader: &mut &[u8]) -> Result<T, SolanaError> {
    T::deserialize_reader(reader).map_err(|e| SolanaError::DecodeFailed(e.to_string()))
}

fn read_exact(reader: &mut &[u8], buf: &mut [u8]) -> Result<(), SolanaError> {
    reader
        .read_exact(buf)
        .map_err(|e| SolanaError::DecodeFailed(e.to_string()))
}

fn hex_le(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn json_number_or_string(f: f64) -> Value {
    serde_json::Number::from_f64(f)
        .map(Value::Number)
        .unwrap_or_else(|| Value::String(f.to_string()))
}
