use std::collections::HashMap;
use std::path::PathBuf;

use ilold_session_core::exploration::session::ExplorationSession;
use ilold_solana_core::execute::VmHost;
use ilold_solana_core::exploration::{execute_call, SolanaCommandResult};
use ilold_solana_core::idl::parse_idl;
use ilold_solana_core::model::ProgramDef;
use ilold_solana_core::overlay::RuntimeOverlay;
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
fn overlay_failed_per_ix_increments_on_real_callfailed_flow() {
    let idl = parse_idl(LEVER_JSON).expect("parse lever idl");
    let program = ProgramDef::from_idl(idl).expect("build ProgramDef");
    let so_bytes = read_lever_so();
    let mut vm = VmHost::boot(vec![(program.program_id, so_bytes)]).expect("boot vm");

    let admin = Keypair::new();
    vm.svm_mut()
        .airdrop(&admin.pubkey(), 5_000_000_000)
        .expect("airdrop admin");

    let mut users: HashMap<String, Keypair> = HashMap::new();
    users.insert("admin".into(), admin.insecure_clone());

    let mut session = ExplorationSession::new("lever", "ilold");

    // switch_power against an uninitialized `power` account — Anchor rejects
    // because the account holds zero bytes (no PowerStatus discriminator), so
    // add_solana_step returns step_index: None and execute_call must surface
    // CallFailed. Real flow exercising the CallFailed branch end-to-end.
    let stray = Keypair::new();
    let mut accounts_in: HashMap<String, String> = HashMap::new();
    accounts_in.insert("power".into(), stray.pubkey().to_string());

    let result = execute_call(
        &program,
        "switch_power",
        serde_json::json!({"name": "claude"}),
        accounts_in,
        vec![],
        &users,
        &mut session,
        &mut vm,
        "2026-05-09T00:00:00Z",
    );

    assert!(
        matches!(result, SolanaCommandResult::CallFailed { .. }),
        "expected CallFailed, got {result:?}"
    );
    assert!(
        session.steps.is_empty(),
        "failed Call must not push a session step"
    );
    assert_eq!(
        session.failed_calls_per_ix.get("switch_power").copied(),
        Some(1),
        "failed_calls_per_ix must be incremented from execute_call CallFailed branch"
    );

    let overlay = RuntimeOverlay::from_session(&session);
    assert_eq!(
        overlay.failed_per_ix.get("switch_power").copied(),
        Some(1),
        "RuntimeOverlay::from_session must surface failed_calls_per_ix"
    );
    assert!(
        overlay.calls_per_ix.is_empty(),
        "no successful Call happened, calls_per_ix should stay empty"
    );
}
