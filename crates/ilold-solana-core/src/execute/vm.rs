use litesvm::LiteSVM;
use solana_address::Address;
use solana_clock::Clock;
use solana_keypair::Keypair;
use solana_signer::Signer;

use crate::error::SolanaError;

pub const DEFAULT_PAYER_LAMPORTS: u64 = 1_000_000_000_000;

pub struct VmHost {
    svm: LiteSVM,
    payer: Keypair,
    programs: Vec<(Address, Vec<u8>)>,
}

impl VmHost {
    pub fn boot(programs: Vec<(Address, Vec<u8>)>) -> Result<Self, SolanaError> {
        let mut svm = LiteSVM::new();
        svm.set_sysvar(&Clock::default());

        let payer = Keypair::new();
        svm.airdrop(&payer.pubkey(), DEFAULT_PAYER_LAMPORTS)
            .map_err(|meta| {
                SolanaError::VmBootFailed(format!("airdrop payer: {:?}", meta.err))
            })?;

        for (program_id, bytes) in &programs {
            svm.add_program(*program_id, bytes).map_err(|e| {
                SolanaError::VmBootFailed(format!(
                    "add_program {program_id}: {e:?}"
                ))
            })?;
        }

        Ok(Self { svm, payer, programs })
    }

    pub(crate) fn programs(&self) -> &[(Address, Vec<u8>)] {
        &self.programs
    }

    pub(crate) fn from_parts(
        svm: LiteSVM,
        payer: Keypair,
        programs: Vec<(Address, Vec<u8>)>,
    ) -> Self {
        Self { svm, payer, programs }
    }

    pub fn payer(&self) -> &Keypair {
        &self.payer
    }

    pub fn payer_pubkey(&self) -> Address {
        self.payer.pubkey()
    }

    pub fn svm(&self) -> &LiteSVM {
        &self.svm
    }

    pub fn svm_mut(&mut self) -> &mut LiteSVM {
        &mut self.svm
    }

    pub fn airdrop(&mut self, address: Address, lamports: u64) -> Result<(), SolanaError> {
        self.svm.airdrop(&address, lamports).map(|_| ()).map_err(|meta| {
            SolanaError::VmOperationFailed(format!("airdrop: {:?}", meta.err))
        })
    }

    pub fn balance(&self, address: &Address) -> u64 {
        self.svm.get_balance(address).unwrap_or(0)
    }

    pub fn warp_clock(&mut self, slot: u64, unix_timestamp: i64) {
        let mut clock = self.svm.get_sysvar::<Clock>();
        clock.slot = slot;
        clock.unix_timestamp = unix_timestamp;
        self.svm.set_sysvar(&clock);
    }

    pub fn clock(&self) -> Clock {
        self.svm.get_sysvar::<Clock>()
    }
}
