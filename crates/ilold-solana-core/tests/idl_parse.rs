use std::path::Path;

use anchor_lang_idl::types::{IdlInstructionAccountItem, IdlSeed};
use ilold_solana_core::error::SolanaError;
use ilold_solana_core::idl::{parse_idl, parse_idl_dir};

#[test]
fn parse_lever_basic_shape() {
    let json = include_str!("fixtures/lever.json");
    let idl = parse_idl(json).expect("lever IDL should parse");

    assert_eq!(idl.metadata.name, "lever");
    assert_eq!(idl.address, "E64FVeubGC4NPNF2UBJYX4AkrVowf74fRJD9q6YhwstN");
    assert_eq!(idl.instructions.len(), 2);
    assert_eq!(idl.instructions[0].name, "initialize");
    assert_eq!(idl.instructions[1].name, "switch_power");
    assert_eq!(idl.accounts.len(), 1);
    assert_eq!(idl.accounts[0].name, "PowerStatus");
    assert_eq!(idl.instructions[0].discriminator.len(), 8);
    assert_eq!(idl.accounts[0].discriminator.len(), 8);
}

#[test]
fn parse_relations_has_pdas_and_composites() {
    let json = include_str!("fixtures/relations.json");
    let idl = parse_idl(json).expect("relations IDL should parse");

    assert_eq!(idl.metadata.name, "relations_derivation");

    let pdas: Vec<_> = idl
        .instructions
        .iter()
        .flat_map(|ix| ix.accounts.iter())
        .filter_map(|a| match a {
            IdlInstructionAccountItem::Single(s) => s.pda.as_ref(),
            IdlInstructionAccountItem::Composite(_) => None,
        })
        .collect();
    assert!(pdas.len() >= 2, "expected multiple PDAs, got {}", pdas.len());

    let composite_count = idl
        .instructions
        .iter()
        .flat_map(|ix| ix.accounts.iter())
        .filter(|a| matches!(a, IdlInstructionAccountItem::Composite(_)))
        .count();
    assert!(composite_count >= 1, "expected at least one composite account group");

    let has_const_seed = pdas
        .iter()
        .flat_map(|p| &p.seeds)
        .any(|s| matches!(s, IdlSeed::Const(_)));
    assert!(has_const_seed, "expected at least one Const seed");
}

#[test]
fn parse_idl_dir_reads_all_jsons_sorted() {
    let dir = Path::new("tests/fixtures");
    let idls = parse_idl_dir(dir).expect("fixtures dir should parse");

    assert_eq!(idls.len(), 2);
    assert_eq!(idls[0].0.file_name().unwrap(), "lever.json");
    assert_eq!(idls[1].0.file_name().unwrap(), "relations.json");
    assert_eq!(idls[0].1.metadata.name, "lever");
    assert_eq!(idls[1].1.metadata.name, "relations_derivation");
}

#[test]
fn parse_idl_invalid_json_returns_parse_error() {
    let err = parse_idl("not valid json {{{").unwrap_err();
    assert!(matches!(err, SolanaError::IdlParseFailed(_)));
}

#[test]
fn parse_idl_dir_missing_returns_read_error() {
    let err = parse_idl_dir(Path::new("/nonexistent/path/__ilold_test__")).unwrap_err();
    assert!(matches!(err, SolanaError::IdlReadFailed { .. }));
}
