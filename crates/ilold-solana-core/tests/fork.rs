use ilold_solana_core::execute::VmHost;
use solana_keypair::Keypair;
use solana_signer::Signer;

#[test]
fn snapshot_then_restore_preserves_payer_and_clock() {
    let mut host = VmHost::boot(Vec::new()).unwrap();
    host.warp_clock(123, 1_700_000_000);

    let snap = host.snapshot();
    let restored = VmHost::restore(snap).unwrap();

    assert_eq!(restored.payer_pubkey(), host.payer_pubkey());
    let clock = restored.clock();
    assert_eq!(clock.slot, 123);
    assert_eq!(clock.unix_timestamp, 1_700_000_000);
}

#[test]
fn fork_branches_diverge_independently() {
    let mut main = VmHost::boot(Vec::new()).unwrap();

    let alice = Keypair::new().pubkey();
    main.airdrop(alice, 1_000_000_000).unwrap();

    let snap = main.snapshot();

    let bob = Keypair::new().pubkey();
    main.airdrop(bob, 2_000_000_000).unwrap();

    let mut branch = VmHost::restore(snap).unwrap();
    let charlie = Keypair::new().pubkey();
    branch.airdrop(charlie, 3_000_000_000).unwrap();

    println!("\n=== fork showcase ===");
    println!("main branch:");
    println!("  alice   = {} lamports", main.balance(&alice));
    println!("  bob     = {} lamports", main.balance(&bob));
    println!("  charlie = {} lamports", main.balance(&charlie));
    println!("forked branch:");
    println!("  alice   = {} lamports", branch.balance(&alice));
    println!("  bob     = {} lamports", branch.balance(&bob));
    println!("  charlie = {} lamports", branch.balance(&charlie));

    assert_eq!(main.balance(&alice), 1_000_000_000);
    assert_eq!(main.balance(&bob), 2_000_000_000);
    assert_eq!(main.balance(&charlie), 0);

    assert_eq!(branch.balance(&alice), 1_000_000_000);
    assert_eq!(branch.balance(&bob), 0);
    assert_eq!(branch.balance(&charlie), 3_000_000_000);
}

#[test]
fn restore_with_invalid_payer_bytes_fails() {
    let mut snap = VmHost::boot(Vec::new()).unwrap().snapshot();
    snap.payer_bytes = [0u8; 64];

    match VmHost::restore(snap) {
        Ok(_) => panic!("expected restore to fail with zeroed payer bytes"),
        Err(e) => assert!(
            matches!(e, ilold_solana_core::error::SolanaError::VmBootFailed(_)),
            "got {e:?}"
        ),
    }
}
