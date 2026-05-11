use std::collections::HashMap;

use anchor_lang_idl::types::IdlType;
use ilold_solana_core::error::SolanaError;
use ilold_solana_core::execute::derive_pda;
use ilold_solana_core::model::{PdaSpec, SeedSpec};
use serde_json::json;
use solana_address::Address;

fn fixed_program_id() -> Address {
    "E64FVeubGC4NPNF2UBJYX4AkrVowf74fRJD9q6YhwstN"
        .parse()
        .unwrap()
}

#[test]
fn const_seed_matches_native_find_program_address() {
    let program = fixed_program_id();
    let spec = PdaSpec {
        seeds: vec![SeedSpec::Const {
            value: b"vault".to_vec(),
        }],
        program: None,
        bump_arg: None,
    };

    let (got_pk, got_bump) = derive_pda(&spec, program, &json!({}), &HashMap::new()).unwrap();
    let (expected_pk, expected_bump) = Address::find_program_address(&[b"vault"], &program);

    assert_eq!(got_pk, expected_pk);
    assert_eq!(got_bump, expected_bump);
}

#[test]
fn string_arg_seed_uses_utf8_bytes() {
    let program = fixed_program_id();
    let spec = PdaSpec {
        seeds: vec![
            SeedSpec::Const {
                value: b"user".to_vec(),
            },
            SeedSpec::Arg {
                path: "name".into(),
                ty: IdlType::String,
            },
        ],
        program: None,
        bump_arg: None,
    };
    let args = json!({"name": "alice"});

    let (got, _bump) = derive_pda(&spec, program, &args, &HashMap::new()).unwrap();
    let (expected, _) = Address::find_program_address(&[b"user", b"alice"], &program);
    assert_eq!(got, expected);
}

#[test]
fn u64_arg_seed_uses_le_bytes() {
    let program = fixed_program_id();
    let spec = PdaSpec {
        seeds: vec![SeedSpec::Arg {
            path: "id".into(),
            ty: IdlType::U64,
        }],
        program: None,
        bump_arg: None,
    };
    let args = json!({"id": 42_u64});

    let (got, _) = derive_pda(&spec, program, &args, &HashMap::new()).unwrap();
    let (expected, _) = Address::find_program_address(&[&42_u64.to_le_bytes()], &program);
    assert_eq!(got, expected);
}

#[test]
fn account_seed_uses_pubkey_bytes() {
    let program = fixed_program_id();
    let user_pk: Address = "11111111111111111111111111111111".parse().unwrap();
    let spec = PdaSpec {
        seeds: vec![SeedSpec::Account {
            path: "user".into(),
        }],
        program: None,
        bump_arg: None,
    };
    let mut accounts = HashMap::new();
    accounts.insert("user".to_string(), user_pk);

    let (got, _) = derive_pda(&spec, program, &json!({}), &accounts).unwrap();
    let (expected, _) = Address::find_program_address(&[&user_pk.to_bytes()], &program);
    assert_eq!(got, expected);
}

#[test]
fn missing_arg_returns_seed_arg_unresolved() {
    let program = fixed_program_id();
    let spec = PdaSpec {
        seeds: vec![SeedSpec::Arg {
            path: "missing".into(),
            ty: IdlType::String,
        }],
        program: None,
        bump_arg: None,
    };

    let err = derive_pda(&spec, program, &json!({}), &HashMap::new()).unwrap_err();
    assert!(matches!(
        err,
        SolanaError::SeedArgUnresolved { ref path } if path == "missing"
    ));
}

#[test]
fn arg_type_mismatch_returns_typed_error() {
    let program = fixed_program_id();
    let spec = PdaSpec {
        seeds: vec![SeedSpec::Arg {
            path: "name".into(),
            ty: IdlType::String,
        }],
        program: None,
        bump_arg: None,
    };
    let args = json!({"name": 123});

    let err = derive_pda(&spec, program, &args, &HashMap::new()).unwrap_err();
    assert!(matches!(err, SolanaError::SeedTypeMismatch { .. }));
}

#[test]
fn arg_in_program_slot_is_rejected() {
    let program = fixed_program_id();
    let spec = PdaSpec {
        seeds: vec![SeedSpec::Const {
            value: b"x".to_vec(),
        }],
        program: Some(SeedSpec::Arg {
            path: "p".into(),
            ty: IdlType::Pubkey,
        }),
        bump_arg: None,
    };

    let err = derive_pda(&spec, program, &json!({}), &HashMap::new()).unwrap_err();
    assert!(matches!(err, SolanaError::PdaProgramArgUnsupported { .. }));
}
