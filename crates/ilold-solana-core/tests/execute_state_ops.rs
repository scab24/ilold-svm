use std::collections::HashMap;

use ilold_session_core::exploration::session::ExplorationSession;
use ilold_session_core::journal::types::{ReviewStatus, Severity};
use ilold_solana_core::execute::VmHost;
use ilold_solana_core::exploration::{
    execute_airdrop, execute_finding, execute_note, execute_status, execute_time_warp,
    execute_users, execute_users_new, SolanaCommandResult,
};
use ilold_solana_core::idl::parse_idl;
use ilold_solana_core::model::ProgramDef;
use solana_keypair::Keypair;

const LEVER_JSON: &str = include_str!("fixtures/lever.json");

fn empty_vm() -> VmHost {
    VmHost::boot(Vec::new()).expect("boot empty vm")
}

fn lever_program() -> ProgramDef {
    let idl = parse_idl(LEVER_JSON).unwrap();
    ProgramDef::from_idl(idl).unwrap()
}

#[test]
fn users_new_generates_keypair_and_airdrops() {
    let mut vm = empty_vm();
    let mut users = HashMap::<String, Keypair>::new();

    let result = execute_users_new("alice".into(), 5_000_000_000, &mut users, &mut vm);
    match result {
        SolanaCommandResult::UserCreated { name, pubkey, lamports } => {
            assert_eq!(name, "alice");
            assert!(!pubkey.is_empty());
            assert_eq!(lamports, 5_000_000_000);
        }
        other => panic!("expected UserCreated, got {other:?}"),
    }
    assert!(users.contains_key("alice"));

    let dup = execute_users_new("alice".into(), 0, &mut users, &mut vm);
    assert!(matches!(dup, SolanaCommandResult::Error { .. }));
}

#[test]
fn users_list_shows_balances() {
    let mut vm = empty_vm();
    let mut users = HashMap::<String, Keypair>::new();
    execute_users_new("bob".into(), 2_000_000_000, &mut users, &mut vm);
    execute_users_new("admin".into(), 8_000_000_000, &mut users, &mut vm);

    match execute_users(&users, &vm) {
        SolanaCommandResult::UserList { users: list } => {
            assert_eq!(list.len(), 2);
            let admin = list.iter().find(|u| u.name == "admin").unwrap();
            assert_eq!(admin.lamports, 8_000_000_000);
            let bob = list.iter().find(|u| u.name == "bob").unwrap();
            assert_eq!(bob.lamports, 2_000_000_000);
        }
        other => panic!("expected UserList, got {other:?}"),
    }
}

#[test]
fn airdrop_unknown_user_errors() {
    let mut vm = empty_vm();
    let users = HashMap::<String, Keypair>::new();
    let result = execute_airdrop("ghost", 1_000, &users, &mut vm);
    assert!(matches!(result, SolanaCommandResult::Error { .. }));
}

#[test]
fn airdrop_known_user_increases_balance() {
    let mut vm = empty_vm();
    let mut users = HashMap::<String, Keypair>::new();
    execute_users_new("bob".into(), 1_000_000_000, &mut users, &mut vm);

    match execute_airdrop("bob", 500_000_000, &users, &mut vm) {
        SolanaCommandResult::Airdropped { name, total_lamports, .. } => {
            assert_eq!(name, "bob");
            assert_eq!(total_lamports, 1_500_000_000);
        }
        other => panic!("expected Airdropped, got {other:?}"),
    }
}

#[test]
fn time_warp_advances_clock() {
    let mut vm = empty_vm();
    let before = vm.clock();
    let result = execute_time_warp(86_400, &mut vm);
    match result {
        SolanaCommandResult::TimeWarped { unix_timestamp, slot } => {
            assert_eq!(unix_timestamp, before.unix_timestamp + 86_400);
            assert_eq!(slot, before.slot + 86_400);
        }
        other => panic!("expected TimeWarped, got {other:?}"),
    }
    assert_eq!(vm.clock().unix_timestamp, before.unix_timestamp + 86_400);
}

#[test]
fn finding_and_note_and_status_record_journal() {
    let program = lever_program();
    let mut session = ExplorationSession::new(&program.name, "ilold");

    match execute_finding(
        &mut session,
        Severity::High,
        "missing signer check".into(),
        "switch_power should require admin".into(),
        None,
        "2026-05-06T00:00:00Z",
    ) {
        SolanaCommandResult::FindingAdded { id } => assert!(!id.is_empty()),
        other => panic!("expected FindingAdded, got {other:?}"),
    }
    assert_eq!(session.journal.findings.len(), 1);

    match execute_note(&mut session, "looking at switch_power", "2026-05-06T00:00:01Z") {
        SolanaCommandResult::NoteAdded => {}
        other => panic!("expected NoteAdded, got {other:?}"),
    }

    match execute_status(
        &mut session,
        &program,
        "switch_power",
        ReviewStatus::Reviewed,
        "2026-05-06T00:00:02Z",
    ) {
        SolanaCommandResult::StatusUpdated => {}
        other => panic!("expected StatusUpdated, got {other:?}"),
    }
    assert_eq!(
        session.journal.function_status.get("switch_power"),
        Some(&ReviewStatus::Reviewed)
    );

    match execute_status(
        &mut session,
        &program,
        "ghost_ix",
        ReviewStatus::Reviewed,
        "2026-05-06T00:00:03Z",
    ) {
        SolanaCommandResult::Error { message } => assert!(message.contains("ghost_ix")),
        other => panic!("expected Error, got {other:?}"),
    }
}
