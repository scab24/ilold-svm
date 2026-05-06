use std::collections::HashMap;

use ilold_session_core::exploration::session::ExplorationSession;
use ilold_solana_core::execute::VmHost;
use ilold_solana_core::exploration::{
    execute_back, execute_call, execute_clear, SolanaCommandResult,
};
use ilold_solana_core::idl::parse_idl;
use ilold_solana_core::model::ProgramDef;
use solana_keypair::Keypair;
use solana_signer::Signer;

const LEVER_JSON: &str = include_str!("fixtures/lever.json");

fn read_lever_so() -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/programs/lever.so");
    std::fs::read(&path).expect("lever.so missing")
}

#[test]
fn execute_call_initialize_then_switch_power() {
    let idl = parse_idl(LEVER_JSON).unwrap();
    let program = ProgramDef::from_idl(idl).unwrap();
    let mut vm = VmHost::boot(vec![(program.program_id, read_lever_so())]).unwrap();

    let admin = Keypair::new();
    let power = Keypair::new();
    vm.svm_mut().airdrop(&admin.pubkey(), 10_000_000_000).unwrap();
    vm.svm_mut().airdrop(&power.pubkey(), 10_000_000_000).unwrap();

    let mut users = HashMap::new();
    users.insert("admin".into(), admin);
    users.insert("power".into(), power);

    let mut session = ExplorationSession::new(&program.name, "ilold");

    let mut accs = HashMap::new();
    accs.insert("power".into(), "power".to_string());
    accs.insert("user".into(), "admin".to_string());
    accs.insert(
        "system_program".into(),
        "11111111111111111111111111111111".to_string(),
    );

    let init = execute_call(
        &program,
        "initialize",
        serde_json::json!({}),
        accs,
        vec!["admin".into(), "power".into()],
        &users,
        &mut session,
        &mut vm,
        "2026-05-06T00:00:00Z",
    );
    let step_index = match init {
        SolanaCommandResult::StepAdded { step_index, instruction, account_diffs_count, .. } => {
            assert_eq!(instruction, "initialize");
            assert!(account_diffs_count >= 1, "init should mutate at least power account");
            step_index
        }
        other => panic!("expected StepAdded for initialize, got {other:?}"),
    };
    assert_eq!(step_index, 0);

    let mut accs2 = HashMap::new();
    accs2.insert("power".into(), "power".to_string());

    let switch = execute_call(
        &program,
        "switch_power",
        serde_json::json!({"name": "claude"}),
        accs2,
        vec![],
        &users,
        &mut session,
        &mut vm,
        "2026-05-06T00:00:01Z",
    );
    match switch {
        SolanaCommandResult::StepAdded { logs_excerpt, .. } => {
            let joined = logs_excerpt.join("\n");
            assert!(
                joined.contains("pulling the power switch"),
                "expected lever log line, got:\n{joined}"
            );
        }
        other => panic!("expected StepAdded for switch_power, got {other:?}"),
    }

    assert!(matches!(
        execute_back(&mut session),
        SolanaCommandResult::StepRemoved { remaining: 1 }
    ));
    assert!(matches!(
        execute_back(&mut session),
        SolanaCommandResult::StepRemoved { remaining: 0 }
    ));
    assert!(matches!(
        execute_back(&mut session),
        SolanaCommandResult::Error { .. }
    ));

    assert!(matches!(execute_clear(&mut session), SolanaCommandResult::Cleared));
}

#[test]
fn execute_call_rejects_unknown_user_in_account() {
    let idl = parse_idl(LEVER_JSON).unwrap();
    let program = ProgramDef::from_idl(idl).unwrap();
    let mut vm = VmHost::boot(vec![(program.program_id, read_lever_so())]).unwrap();
    let users: HashMap<String, Keypair> = HashMap::new();
    let mut session = ExplorationSession::new(&program.name, "ilold");

    let mut accs = HashMap::new();
    accs.insert("power".into(), "ghost_user".to_string());

    let result = execute_call(
        &program,
        "switch_power",
        serde_json::json!({"name": "x"}),
        accs,
        vec![],
        &users,
        &mut session,
        &mut vm,
        "2026-05-06T00:00:00Z",
    );
    match result {
        SolanaCommandResult::Error { message } => {
            assert!(message.contains("ghost_user"), "got: {message}");
        }
        other => panic!("expected Error, got {other:?}"),
    }
}

#[test]
fn execute_call_rejects_missing_signer_keypair() {
    let idl = parse_idl(LEVER_JSON).unwrap();
    let program = ProgramDef::from_idl(idl).unwrap();
    let mut vm = VmHost::boot(vec![(program.program_id, read_lever_so())]).unwrap();
    let users: HashMap<String, Keypair> = HashMap::new();
    let mut session = ExplorationSession::new(&program.name, "ilold");

    let mut accs = HashMap::new();
    accs.insert("power".into(), Keypair::new().pubkey().to_string());
    accs.insert("user".into(), Keypair::new().pubkey().to_string());
    accs.insert(
        "system_program".into(),
        "11111111111111111111111111111111".to_string(),
    );

    let result = execute_call(
        &program,
        "initialize",
        serde_json::json!({}),
        accs,
        vec![],
        &users,
        &mut session,
        &mut vm,
        "2026-05-06T00:00:00Z",
    );
    match result {
        SolanaCommandResult::Error { message } => {
            assert!(
                message.contains("signer"),
                "expected signer error, got: {message}"
            );
        }
        other => panic!("expected Error, got {other:?}"),
    }
}
