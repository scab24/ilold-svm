use anchor_lang_idl::types::{
    IdlArrayLen, IdlDefinedFields, IdlEnumVariant, IdlField, IdlSerialization, IdlType,
    IdlTypeDef, IdlTypeDefTy,
};
use ilold_solana_core::decode::decode_value;
use ilold_solana_core::encode::encode_value;
use serde_json::{json, Value};

fn types() -> Vec<IdlTypeDef> {
    Vec::new()
}

fn roundtrip(value: Value, ty: &IdlType, types: &[IdlTypeDef]) -> Value {
    let bytes = encode_value(&value, ty, types).expect("encode");
    let mut cursor: &[u8] = &bytes;
    decode_value(&mut cursor, ty, types).expect("decode")
}

#[test]
fn primitives_roundtrip() {
    assert_eq!(roundtrip(json!(true), &IdlType::Bool, &types()), json!(true));
    assert_eq!(roundtrip(json!(42), &IdlType::U8, &types()), json!(42));
    assert_eq!(roundtrip(json!(-7), &IdlType::I32, &types()), json!(-7));
    assert_eq!(roundtrip(json!(1_234_567_890_u64), &IdlType::U64, &types()), json!(1_234_567_890_u64));
}

#[test]
fn u128_roundtrip_as_decimal_string() {
    let value = json!("340282366920938463463374607431768211455");
    let result = roundtrip(value.clone(), &IdlType::U128, &types());
    assert_eq!(result, value);
}

#[test]
fn string_and_bytes_roundtrip() {
    assert_eq!(
        roundtrip(json!("hola"), &IdlType::String, &types()),
        json!("hola")
    );
    let bytes_hex = json!("0102030a");
    assert_eq!(roundtrip(bytes_hex.clone(), &IdlType::Bytes, &types()), bytes_hex);
}

#[test]
fn pubkey_roundtrip() {
    let pk = json!("11111111111111111111111111111111");
    assert_eq!(roundtrip(pk.clone(), &IdlType::Pubkey, &types()), pk);
}

#[test]
fn option_some_and_none_roundtrip() {
    let ty = IdlType::Option(Box::new(IdlType::U32));
    assert_eq!(roundtrip(json!(null), &ty, &types()), json!(null));
    assert_eq!(roundtrip(json!(7), &ty, &types()), json!(7));
}

#[test]
fn vec_and_array_roundtrip() {
    let vec_ty = IdlType::Vec(Box::new(IdlType::U16));
    assert_eq!(roundtrip(json!([10, 20, 30]), &vec_ty, &types()), json!([10, 20, 30]));

    let arr_ty = IdlType::Array(Box::new(IdlType::U32), IdlArrayLen::Value(3));
    assert_eq!(roundtrip(json!([1, 2, 3]), &arr_ty, &types()), json!([1, 2, 3]));
}

#[test]
fn defined_struct_roundtrip() {
    let counter = IdlTypeDef {
        name: "Counter".into(),
        docs: vec![],
        serialization: IdlSerialization::default(),
        repr: None,
        generics: vec![],
        ty: IdlTypeDefTy::Struct {
            fields: Some(IdlDefinedFields::Named(vec![
                IdlField {
                    name: "count".into(),
                    docs: vec![],
                    ty: IdlType::U64,
                },
                IdlField {
                    name: "active".into(),
                    docs: vec![],
                    ty: IdlType::Bool,
                },
            ])),
        },
    };
    let types = vec![counter];
    let ty = IdlType::Defined {
        name: "Counter".into(),
        generics: vec![],
    };
    let value = json!({"count": 99, "active": true});
    assert_eq!(roundtrip(value.clone(), &ty, &types), value);
}

#[test]
fn enum_with_unit_and_payload_roundtrip() {
    let status = IdlTypeDef {
        name: "Status".into(),
        docs: vec![],
        serialization: IdlSerialization::default(),
        repr: None,
        generics: vec![],
        ty: IdlTypeDefTy::Enum {
            variants: vec![
                IdlEnumVariant {
                    name: "Off".into(),
                    fields: None,
                },
                IdlEnumVariant {
                    name: "On".into(),
                    fields: Some(IdlDefinedFields::Named(vec![IdlField {
                        name: "level".into(),
                        docs: vec![],
                        ty: IdlType::U8,
                    }])),
                },
            ],
        },
    };
    let types = vec![status];
    let ty = IdlType::Defined {
        name: "Status".into(),
        generics: vec![],
    };

    let off = json!({"kind": "Off"});
    assert_eq!(roundtrip(off.clone(), &ty, &types), off);

    let on = json!({"kind": "On", "value": {"level": 200}});
    assert_eq!(roundtrip(on.clone(), &ty, &types), on);
}
