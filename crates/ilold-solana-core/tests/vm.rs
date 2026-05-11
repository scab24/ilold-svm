use ilold_solana_core::error::SolanaError;
use ilold_solana_core::execute::{vm::DEFAULT_PAYER_LAMPORTS, VmHost};
use solana_keypair::Keypair;
use solana_signer::Signer;

#[test]
fn boot_empty_vm_funds_payer() {
    let host = VmHost::boot(Vec::new()).expect("empty boot should succeed");
    assert_eq!(host.balance(&host.payer_pubkey()), DEFAULT_PAYER_LAMPORTS);
}

#[test]
fn airdrop_credits_target_address() {
    let mut host = VmHost::boot(Vec::new()).unwrap();
    let target = Keypair::new().pubkey();

    let amount = 5_000_000_000;
    host.airdrop(target, amount).unwrap();

    assert_eq!(host.balance(&target), amount);
}

#[test]
fn warp_clock_updates_slot_and_timestamp() {
    let mut host = VmHost::boot(Vec::new()).unwrap();

    host.warp_clock(1_000, 1_700_000_000);
    let clock = host.clock();

    assert_eq!(clock.slot, 1_000);
    assert_eq!(clock.unix_timestamp, 1_700_000_000);
}

#[test]
fn boot_with_invalid_program_bytes_returns_boot_failed() {
    let fake_program_id = solana_keypair::Keypair::new().pubkey();
    let invalid_elf = vec![0u8; 16];

    match VmHost::boot(vec![(fake_program_id, invalid_elf)]) {
        Ok(_) => panic!("expected VmBootFailed for invalid ELF bytes"),
        Err(e) => assert!(matches!(e, SolanaError::VmBootFailed(_))),
    }
}
