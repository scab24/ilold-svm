use ilold_session_core::exploration::session::ExplorationSession;
use ilold_solana_core::exploration::{
    execute_funcs, execute_pda, execute_session, SolanaCommandResult,
};
use ilold_solana_core::idl::parse_idl;
use ilold_solana_core::model::ProgramDef;

const LEVER_JSON: &str = include_str!("fixtures/lever.json");
const RELATIONS_JSON: &str = include_str!("fixtures/relations.json");

fn lever_program() -> ProgramDef {
    let idl = parse_idl(LEVER_JSON).expect("parse lever");
    ProgramDef::from_idl(idl).expect("build lever ProgramDef")
}

fn relations_program() -> ProgramDef {
    let idl = parse_idl(RELATIONS_JSON).expect("parse relations");
    ProgramDef::from_idl(idl).expect("build relations ProgramDef")
}

#[test]
fn funcs_lists_lever_instructions() {
    let program = lever_program();
    let result = execute_funcs(&program);
    let items = match result {
        SolanaCommandResult::InstructionList { items } => items,
        other => panic!("expected InstructionList, got {other:?}"),
    };
    assert_eq!(items.len(), 2);
    let names: Vec<_> = items.iter().map(|i| i.name.as_str()).collect();
    assert!(names.contains(&"initialize"));
    assert!(names.contains(&"switch_power"));

    let init = items.iter().find(|i| i.name == "initialize").unwrap();
    assert!(init.signers.iter().any(|s| s == "user"));
    assert!(init.signers.iter().any(|s| s == "power"));
    assert_eq!(init.has_pdas, false);
}

#[test]
fn pda_lists_relations_pdas_symbolically() {
    let program = relations_program();
    let with_pdas: Vec<_> = program
        .instructions
        .iter()
        .find(|ix| ix.accounts.iter().any(|a| a.pda.is_some()))
        .into_iter()
        .collect();
    let probe = with_pdas
        .first()
        .expect("relations IDL should declare at least one ix with PDAs");

    let result = execute_pda(&program, &probe.name);
    let pdas = match result {
        SolanaCommandResult::PdaList { pdas, .. } => pdas,
        other => panic!("expected PdaList, got {other:?}"),
    };
    assert!(!pdas.is_empty(), "expected at least one PDA in {}", probe.name);
    let entry = &pdas[0];
    assert!(!entry.account_name.is_empty());
    assert!(!entry.seeds.is_empty(), "PDAs must have seeds");
    assert!(
        !entry.program.is_empty(),
        "PDA program must resolve to a string"
    );
}

#[test]
fn pda_unknown_instruction_returns_error() {
    let program = lever_program();
    let result = execute_pda(&program, "does_not_exist");
    match result {
        SolanaCommandResult::Error { message } => {
            assert!(message.contains("does_not_exist"));
        }
        other => panic!("expected Error, got {other:?}"),
    }
}

#[test]
fn session_view_reports_empty_on_fresh_session() {
    let program = lever_program();
    let session = ExplorationSession::new(&program.name, "ilold");
    let result = execute_session(&session, &program, "main");
    match result {
        SolanaCommandResult::SessionView {
            program: prog,
            scenario,
            steps,
            findings_count,
        } => {
            assert_eq!(prog, program.name);
            assert_eq!(scenario, "main");
            assert!(steps.is_empty());
            assert_eq!(findings_count, 0);
        }
        other => panic!("expected SessionView, got {other:?}"),
    }
}
