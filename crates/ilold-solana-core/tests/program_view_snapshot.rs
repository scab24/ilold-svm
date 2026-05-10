use std::fs;
use std::path::{Path, PathBuf};

use ilold_session_core::exploration::session::ExplorationSession;
use ilold_solana_core::exploration::{
    execute_funcs, execute_pda, execute_who, SolanaCommandResult,
};
use ilold_solana_core::idl::parse_idl;
use ilold_solana_core::model::ProgramDef;
use ilold_solana_core::overlay::RuntimeOverlay;

const LEVER_JSON: &str = include_str!("fixtures/lever.json");
const RELATIONS_JSON: &str = include_str!("fixtures/relations.json");
const STAKING_JSON: &str = include_str!("../../../tests/fixtures/solana/staking/idls/staking.json");

fn snapshot_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
        .join("funcs_who_pda_baseline")
}

fn regen_enabled() -> bool {
    std::env::var("ILOLD_REGEN_SNAPSHOTS").is_ok()
}

fn assert_snapshot(name: &str, value: &SolanaCommandResult) {
    let json = serde_json::to_string_pretty(value).expect("serialize result");
    let path = snapshot_root().join(format!("{name}.json"));
    if regen_enabled() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, format!("{json}\n")).unwrap();
        return;
    }
    let expected = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing snapshot {}: {e}", path.display()));
    let expected_trimmed = expected.trim_end_matches('\n');
    let json_trimmed = json.trim_end_matches('\n');
    assert_eq!(
        json_trimmed, expected_trimmed,
        "snapshot drift in {name}\nstored at {}",
        path.display()
    );
}

fn staking_program() -> ProgramDef {
    ProgramDef::from_idl(parse_idl(STAKING_JSON).expect("parse staking"))
        .expect("build staking ProgramDef")
}

fn lever_program() -> ProgramDef {
    ProgramDef::from_idl(parse_idl(LEVER_JSON).expect("parse lever"))
        .expect("build lever ProgramDef")
}

fn relations_program() -> ProgramDef {
    ProgramDef::from_idl(parse_idl(RELATIONS_JSON).expect("parse relations"))
        .expect("build relations ProgramDef")
}

#[test]
fn snapshot_funcs_staking() {
    let program = staking_program();
    assert_snapshot("staking_funcs", &execute_funcs(&program));
}

#[test]
fn snapshot_funcs_lever() {
    let program = lever_program();
    assert_snapshot("lever_funcs", &execute_funcs(&program));
}

#[test]
fn snapshot_who_pool() {
    let program = staking_program();
    assert_snapshot("staking_who_pool", &execute_who(&program, "Pool"));
}

#[test]
fn snapshot_who_user_stake() {
    let program = staking_program();
    assert_snapshot("staking_who_user_stake", &execute_who(&program, "UserStake"));
}

#[test]
fn snapshot_pda_staking_per_ix() {
    let program = staking_program();
    for ix in &program.instructions {
        let result = execute_pda(&program, &ix.name);
        assert_snapshot(&format!("staking_pda_{}", ix.name), &result);
    }
}

#[test]
fn snapshot_pda_lever_per_ix() {
    let program = lever_program();
    for ix in &program.instructions {
        let result = execute_pda(&program, &ix.name);
        assert_snapshot(&format!("lever_pda_{}", ix.name), &result);
    }
}

#[test]
fn snapshot_pda_relations_per_ix() {
    let program = relations_program();
    for ix in &program.instructions {
        let result = execute_pda(&program, &ix.name);
        assert_snapshot(&format!("relations_pda_{}", ix.name), &result);
    }
}

#[test]
fn program_view_wire_format_staking() {
    let view = staking_program().compute_view();
    let json = serde_json::to_string_pretty(&view).expect("serialize ProgramView");
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
        .join("staking_view.json");
    if regen_enabled() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, format!("{json}\n")).unwrap();
        return;
    }
    let expected = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing wire-format snapshot {}: {e}", path.display()));
    let expected_trimmed = expected.trim_end_matches('\n');
    let json_trimmed = json.trim_end_matches('\n');
    assert_eq!(
        json_trimmed, expected_trimmed,
        "ProgramView wire-format drift; if intentional, regen with ILOLD_REGEN_SNAPSHOTS=1"
    );
}

#[test]
fn runtime_overlay_wire_format_empty_staking() {
    let session = ExplorationSession::new("staking", "ilold");
    let mut overlay = RuntimeOverlay::from_session(&session);
    overlay.scenario = "main".to_string();
    let json = serde_json::to_string_pretty(&overlay).expect("serialize RuntimeOverlay");
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
        .join("staking_overlay.json");
    if regen_enabled() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, format!("{json}\n")).unwrap();
        return;
    }
    let expected = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing wire-format snapshot {}: {e}", path.display()));
    let expected_trimmed = expected.trim_end_matches('\n');
    let json_trimmed = json.trim_end_matches('\n');
    assert_eq!(
        json_trimmed, expected_trimmed,
        "RuntimeOverlay wire-format drift; if intentional, regen with ILOLD_REGEN_SNAPSHOTS=1"
    );
}
