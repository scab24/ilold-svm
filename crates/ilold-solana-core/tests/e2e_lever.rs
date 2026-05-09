use std::collections::HashMap;
use std::path::PathBuf;

use ilold_session_core::exploration::session::ExplorationSession;
use ilold_solana_core::execute::VmHost;
use ilold_solana_core::exploration::add_solana_step;
use ilold_solana_core::idl::parse_idl;
use ilold_solana_core::model::ProgramDef;
use solana_account::Account;
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
fn add_solana_step_runs_switch_power_against_real_program() {
    let idl = parse_idl(LEVER_JSON).expect("parse lever idl");
    let program = ProgramDef::from_idl(idl).expect("build ProgramDef");
    let so_bytes = read_lever_so();

    let mut vm = VmHost::boot(vec![(program.program_id, so_bytes)]).expect("boot vm");

    let power_status_def = program
        .account_types
        .iter()
        .find(|a| a.name == "PowerStatus")
        .expect("PowerStatus account in IDL");

    let power_kp = Keypair::new();
    let power_pk = power_kp.pubkey();
    let mut data = Vec::with_capacity(9);
    data.extend_from_slice(&power_status_def.discriminator);
    data.push(0u8);
    let lamports = vm.svm().minimum_balance_for_rent_exemption(data.len());
    let acc = Account {
        lamports,
        data,
        owner: program.program_id,
        executable: false,
        rent_epoch: 0,
    };
    vm.svm_mut().set_account(power_pk, acc).expect("seed power account");

    let mut accounts: HashMap<String, Address> = HashMap::new();
    accounts.insert("power".into(), power_pk);

    let switch = program
        .instructions
        .iter()
        .find(|ix| ix.name == "switch_power")
        .expect("switch_power ix");

    let mut session = ExplorationSession::new("lever", "ilold");
    let step = add_solana_step(
        &mut session,
        &program,
        switch,
        &mut vm,
        serde_json::json!({"name": "claude"}),
        accounts,
        &[],
        "2026-05-06T00:00:00Z",
        None,
    )
    .expect("add_solana_step");

    assert_eq!(step.function, "switch_power");
    let trace = step.runtime_trace.as_ref().expect("runtime_trace populated");
    assert!(
        trace.get("error").map(|v| v.is_null()).unwrap_or(true),
        "transaction errored: {:?}",
        trace.get("error")
    );
    let logs = trace
        .get("logs")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let joined = logs
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        joined.contains("pulling the power switch"),
        "expected lever log line, got:\n{joined}"
    );

    let after = vm.svm().get_account(&power_pk).expect("power account after");
    assert_eq!(after.data[8], 1, "is_on flag should flip to true");
}
