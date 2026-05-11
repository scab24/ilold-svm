use anchor_lang_idl::types::{
    Idl, IdlAccount, IdlArrayLen, IdlDefinedFields, IdlEnumVariant, IdlField, IdlMetadata,
    IdlSerialization, IdlType, IdlTypeDef, IdlTypeDefTy,
};
use ilold_solana_core::decode::{decode_account, decode_value};
use ilold_solana_core::error::SolanaError;
use serde_json::json;

fn types() -> Vec<IdlTypeDef> {
    Vec::new()
}

#[test]
fn primitives_decode_correctly() {
    let mut bytes: &[u8] = &[1u8];
    assert_eq!(decode_value(&mut bytes, &IdlType::Bool, &types()).unwrap(), json!(true));

    let mut bytes: &[u8] = &[42u8];
    assert_eq!(decode_value(&mut bytes, &IdlType::U8, &types()).unwrap(), json!(42));

    let mut bytes: &[u8] = &42u64.to_le_bytes();
    assert_eq!(decode_value(&mut bytes, &IdlType::U64, &types()).unwrap(), json!(42));

    let mut bytes: &[u8] = &(-7i32).to_le_bytes();
    assert_eq!(decode_value(&mut bytes, &IdlType::I32, &types()).unwrap(), json!(-7));
}

#[test]
fn u128_renders_as_decimal_string() {
    let value: u128 = 340_282_366_920_938_463_463_374_607_431_768_211_455;
    let mut bytes: &[u8] = &value.to_le_bytes();
    assert_eq!(
        decode_value(&mut bytes, &IdlType::U128, &types()).unwrap(),
        json!(value.to_string())
    );
}

#[test]
fn string_decodes_with_length_prefix() {
    let mut buf = Vec::new();
    let s = "hola";
    buf.extend(&(s.len() as u32).to_le_bytes());
    buf.extend(s.as_bytes());
    let mut bytes: &[u8] = &buf;
    assert_eq!(
        decode_value(&mut bytes, &IdlType::String, &types()).unwrap(),
        json!("hola")
    );
}

#[test]
fn pubkey_renders_base58() {
    let bytes = [0u8; 32];
    let mut slice: &[u8] = &bytes;
    let v = decode_value(&mut slice, &IdlType::Pubkey, &types()).unwrap();
    assert_eq!(v, json!("11111111111111111111111111111111"));
}

#[test]
fn option_some_and_none() {
    let mut buf: Vec<u8> = vec![0];
    let mut bytes: &[u8] = &buf;
    let ty = IdlType::Option(Box::new(IdlType::U32));
    assert_eq!(decode_value(&mut bytes, &ty, &types()).unwrap(), json!(null));

    buf = vec![1];
    buf.extend(&7u32.to_le_bytes());
    let mut bytes: &[u8] = &buf;
    assert_eq!(decode_value(&mut bytes, &ty, &types()).unwrap(), json!(7));
}

#[test]
fn vec_u32_with_length_prefix() {
    let mut buf = (3u32).to_le_bytes().to_vec();
    for v in [10u32, 20, 30] {
        buf.extend(&v.to_le_bytes());
    }
    let ty = IdlType::Vec(Box::new(IdlType::U32));
    let mut bytes: &[u8] = &buf;
    assert_eq!(
        decode_value(&mut bytes, &ty, &types()).unwrap(),
        json!([10, 20, 30])
    );
}

#[test]
fn array_fixed_length() {
    let mut buf = Vec::new();
    for v in [1u32, 2, 3] {
        buf.extend(&v.to_le_bytes());
    }
    let ty = IdlType::Array(Box::new(IdlType::U32), IdlArrayLen::Value(3));
    let mut bytes: &[u8] = &buf;
    assert_eq!(decode_value(&mut bytes, &ty, &types()).unwrap(), json!([1, 2, 3]));
}

#[test]
fn defined_struct_decodes_named_fields() {
    let counter_typedef = IdlTypeDef {
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
    let types = vec![counter_typedef];
    let ty = IdlType::Defined {
        name: "Counter".into(),
        generics: vec![],
    };

    let mut buf = 99u64.to_le_bytes().to_vec();
    buf.push(1u8);
    let mut bytes: &[u8] = &buf;
    assert_eq!(
        decode_value(&mut bytes, &ty, &types).unwrap(),
        json!({"count": 99, "active": true})
    );
}

#[test]
fn enum_with_unit_and_payload_variants() {
    let status_typedef = IdlTypeDef {
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
    let types = vec![status_typedef];
    let ty = IdlType::Defined {
        name: "Status".into(),
        generics: vec![],
    };

    let mut bytes: &[u8] = &[0u8];
    assert_eq!(
        decode_value(&mut bytes, &ty, &types).unwrap(),
        json!({"kind": "Off"})
    );

    let buf = vec![1u8, 200u8];
    let mut bytes: &[u8] = &buf;
    assert_eq!(
        decode_value(&mut bytes, &ty, &types).unwrap(),
        json!({"kind": "On", "value": {"level": 200}})
    );
}

#[test]
fn decode_account_uses_discriminator_lookup() {
    let counter_typedef = IdlTypeDef {
        name: "Counter".into(),
        docs: vec![],
        serialization: IdlSerialization::default(),
        repr: None,
        generics: vec![],
        ty: IdlTypeDefTy::Struct {
            fields: Some(IdlDefinedFields::Named(vec![IdlField {
                name: "count".into(),
                docs: vec![],
                ty: IdlType::U64,
            }])),
        },
    };
    let disc = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
    let idl = Idl {
        address: "11111111111111111111111111111111".into(),
        metadata: IdlMetadata {
            name: "test".into(),
            version: "0.1.0".into(),
            spec: "0.1.0".into(),
            description: None,
            repository: None,
            dependencies: vec![],
            contact: None,
            deployments: None,
        },
        docs: vec![],
        instructions: vec![],
        accounts: vec![IdlAccount {
            name: "Counter".into(),
            discriminator: disc.clone(),
        }],
        events: vec![],
        errors: vec![],
        types: vec![counter_typedef],
        constants: vec![],
    };

    let mut data = disc.clone();
    data.extend(&123u64.to_le_bytes());
    let decoded = decode_account(&data, &idl).unwrap();
    assert_eq!(decoded.type_name, "Counter");
    assert_eq!(decoded.value, json!({"count": 123}));
}

#[test]
fn decode_account_unknown_discriminator_errors() {
    let idl = Idl {
        address: "11111111111111111111111111111111".into(),
        metadata: IdlMetadata {
            name: "test".into(),
            version: "0.1.0".into(),
            spec: "0.1.0".into(),
            description: None,
            repository: None,
            dependencies: vec![],
            contact: None,
            deployments: None,
        },
        docs: vec![],
        instructions: vec![],
        accounts: vec![],
        events: vec![],
        errors: vec![],
        types: vec![],
        constants: vec![],
    };

    let data = [9u8; 16];
    let err = decode_account(&data, &idl).unwrap_err();
    assert!(matches!(err, SolanaError::UnknownDiscriminator { .. }));
}

#[test]
fn generics_rejected() {
    let ty = IdlType::Generic("T".into());
    let mut bytes: &[u8] = &[0u8];
    let err = decode_value(&mut bytes, &ty, &types()).unwrap_err();
    assert!(matches!(err, SolanaError::UnsupportedGeneric(_)));
}
