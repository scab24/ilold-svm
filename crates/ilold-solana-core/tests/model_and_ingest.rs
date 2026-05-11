use std::fs;

use ilold_solana_core::error::SolanaError;
use ilold_solana_core::idl::parse_idl;
use ilold_solana_core::ingest::{detect, ProjectKind};
use ilold_solana_core::model::{ProgramDef, SeedSpec, SolanaProject};

const LEVER_JSON: &str = include_str!("fixtures/lever.json");
const RELATIONS_JSON: &str = include_str!("fixtures/relations.json");

#[test]
fn program_def_from_lever_idl() {
    let idl = parse_idl(LEVER_JSON).unwrap();
    let program = ProgramDef::from_idl(idl).unwrap();

    assert_eq!(program.name, "lever");
    assert_eq!(program.program_id.to_string(), "E64FVeubGC4NPNF2UBJYX4AkrVowf74fRJD9q6YhwstN");
    assert_eq!(program.instructions.len(), 2);

    let init = &program.instructions[0];
    assert_eq!(init.name, "initialize");
    assert_eq!(init.discriminator.len(), 8);
    assert!(!init.accounts.is_empty());

    let switch = &program.instructions[1];
    assert_eq!(switch.name, "switch_power");
    assert!(!switch.accounts.is_empty());

    assert_eq!(program.account_types.len(), 1);
    assert_eq!(program.account_types[0].name, "PowerStatus");
    assert_eq!(program.account_types[0].discriminator.len(), 8);
}

#[test]
fn program_def_from_relations_flattens_composites_and_maps_seeds() {
    let idl = parse_idl(RELATIONS_JSON).unwrap();
    let program = ProgramDef::from_idl(idl).unwrap();

    let composite_paths: Vec<&str> = program
        .instructions
        .iter()
        .flat_map(|ix| ix.accounts.iter())
        .filter(|a| a.path.contains('.'))
        .map(|a| a.path.as_str())
        .collect();
    assert!(
        !composite_paths.is_empty(),
        "expected at least one dotted path from a composite group"
    );

    let any_const_seed = program
        .instructions
        .iter()
        .flat_map(|ix| ix.accounts.iter())
        .filter_map(|a| a.pda.as_ref())
        .flat_map(|pda| pda.seeds.iter())
        .any(|s| matches!(s, SeedSpec::Const { .. }));
    assert!(any_const_seed, "expected at least one Const seed mapped");
}

#[test]
fn solana_project_index_lookup() {
    let lever = ProgramDef::from_idl(parse_idl(LEVER_JSON).unwrap()).unwrap();
    let relations = ProgramDef::from_idl(parse_idl(RELATIONS_JSON).unwrap()).unwrap();

    let project = SolanaProject::new(vec![lever, relations]);

    assert!(project.find_program("lever").is_some());
    assert!(project.find_program("relations_derivation").is_some());
    assert!(project.find_program("missing").is_none());
}

#[test]
fn detect_anchor_project() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Anchor.toml"), "[programs.localnet]\n").unwrap();

    let detected = detect(dir.path()).unwrap();
    assert_eq!(detected.kind, ProjectKind::Solana);
    assert_eq!(detected.root, dir.path());
    assert!(detected.idl_paths.is_empty());
    assert!(detected.so_paths.is_empty());
}

#[test]
fn detect_solidity_project_via_foundry_toml() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("foundry.toml"), "[profile.default]\n").unwrap();

    let detected = detect(dir.path()).unwrap();
    assert_eq!(detected.kind, ProjectKind::Solidity);
}

#[test]
fn detect_solidity_project_via_sol_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src/Token.sol"),
        "// SPDX-License-Identifier: MIT\npragma solidity ^0.8.0;\ncontract Token {}\n",
    )
    .unwrap();

    let detected = detect(dir.path()).unwrap();
    assert_eq!(detected.kind, ProjectKind::Solidity);
}

#[test]
fn detect_mixed_project_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Anchor.toml"), "[programs.localnet]\n").unwrap();
    fs::write(dir.path().join("foundry.toml"), "[profile.default]\n").unwrap();

    let err = detect(dir.path()).unwrap_err();
    assert!(matches!(err, SolanaError::MixedProject { .. }));
}

#[test]
fn detect_empty_directory_returns_unknown_type() {
    let dir = tempfile::tempdir().unwrap();
    let err = detect(dir.path()).unwrap_err();
    assert!(matches!(err, SolanaError::UnknownProjectType { .. }));
}

#[test]
fn detect_anchor_finds_idls_under_target() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Anchor.toml"), "[programs.localnet]\n").unwrap();
    let idl_dir = dir.path().join("target").join("idl");
    fs::create_dir_all(&idl_dir).unwrap();
    fs::write(idl_dir.join("foo.json"), LEVER_JSON).unwrap();

    let detected = detect(dir.path()).unwrap();
    assert_eq!(detected.kind, ProjectKind::Solana);
    assert_eq!(detected.idl_paths.len(), 1);
    assert!(detected.idl_paths[0].ends_with("foo.json"));
}
