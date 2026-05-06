use std::collections::HashMap;
use std::path::PathBuf;

use ilold_session_core::exploration::session::ExplorationSession;
use ilold_solana_core::execute::VmHost;
use ilold_solana_core::exploration::add_solana_step;
use ilold_solana_core::idl::parse_idl;
use ilold_solana_core::model::ProgramDef;
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signer::Signer;

const LEVER_JSON: &str = include_str!("fixtures/lever.json");

fn lever_so_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/programs/lever.so")
}

fn read_lever_so() -> Vec<u8> {
    std::fs::read(lever_so_path()).expect(
        "lever.so missing — run `cd tests/fixtures/solana/cpi && anchor build` and copy \
         target/deploy/lever.so to crates/ilold-solana-core/tests/programs/lever.so",
    )
}

#[test]
#[ignore = "requires lever.so produced by anchor build"]
fn add_solana_step_executes_initialize_against_real_program() {
    let idl = parse_idl(LEVER_JSON).expect("parse lever idl");
    let program = ProgramDef::from_idl(idl).expect("build ProgramDef");
    let so_bytes = read_lever_so();

    let mut vm = VmHost::boot(vec![(program.program_id, so_bytes)]).expect("boot vm");

    let power_status = Keypair::new();
    let mut accounts: HashMap<String, Address> = HashMap::new();
    accounts.insert("power".into(), power_status.pubkey());
    accounts.insert("user".into(), vm.payer_pubkey());
    accounts.insert(
        "system_program".into(),
        "11111111111111111111111111111111".parse().unwrap(),
    );

    let initialize = program
        .instructions
        .iter()
        .find(|ix| ix.name == "initialize")
        .expect("initialize ix");

    let mut session = ExplorationSession::new("lever", "ilold");
    let step = add_solana_step(
        &mut session,
        &program,
        initialize,
        &mut vm,
        serde_json::json!({}),
        accounts,
        "2026-05-06T00:00:00Z",
    )
    .expect("add_solana_step");

    assert_eq!(step.function, "initialize");
    let trace = step.runtime_trace.as_ref().expect("runtime_trace populated");
    let logs = trace.get("logs").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    assert!(!logs.is_empty(), "expected non-empty logs after initialize");
}
