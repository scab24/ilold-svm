use litesvm::LiteSVM;
use solana_account::{AccountSharedData, ReadableAccount};
use solana_address::Address;
use solana_clock::Clock;
use solana_keypair::Keypair;
use solana_sdk_ids::bpf_loader_upgradeable;
use solana_signer::Signer;

use crate::error::SolanaError;
use crate::execute::vm::VmHost;

const PROGRAMDATA_DISCRIMINATOR_BYTE: u8 = 3;

#[derive(Clone)]
pub struct VmSnapshot {
    pub accounts: Vec<(Address, AccountSharedData)>,
    pub clock: Clock,
    pub payer_bytes: [u8; 64],
    pub programs: Vec<(Address, Vec<u8>)>,
}

impl VmHost {
    pub fn snapshot(&self) -> VmSnapshot {
        let accounts: Vec<(Address, AccountSharedData)> = self
            .svm()
            .accounts_db()
            .inner
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        VmSnapshot {
            accounts,
            clock: self.svm().get_sysvar::<Clock>(),
            payer_bytes: self.payer().to_bytes(),
            programs: self.programs().to_vec(),
        }
    }

    pub fn restore(snap: VmSnapshot) -> Result<Self, SolanaError> {
        let mut svm = LiteSVM::new();
        svm.set_sysvar(&snap.clock);

        for (program_id, bytes) in &snap.programs {
            svm.add_program(*program_id, bytes).map_err(|e| {
                SolanaError::VmBootFailed(format!("restore add_program {program_id}: {e:?}"))
            })?;
        }

        let upgradeable = bpf_loader_upgradeable::id();
        let (programdata, other): (Vec<_>, Vec<_>) =
            snap.accounts.iter().cloned().partition(|(_, acc)| {
                acc.owner() == &upgradeable
                    && acc.data().first() == Some(&PROGRAMDATA_DISCRIMINATOR_BYTE)
            });

        for (pk, acc) in programdata {
            svm.set_account(pk, acc.into()).map_err(|e| {
                SolanaError::VmOperationFailed(format!("restore programdata {pk}: {e:?}"))
            })?;
        }
        for (pk, acc) in other {
            svm.set_account(pk, acc.into())
                .map_err(|e| SolanaError::VmOperationFailed(format!("restore account {pk}: {e:?}")))?;
        }

        let payer = Keypair::try_from(snap.payer_bytes.as_slice())
            .map_err(|_| SolanaError::VmBootFailed("invalid payer bytes in snapshot".into()))?;

        Ok(VmHost::from_parts(svm, payer, snap.programs))
    }
}

impl VmSnapshot {
    pub fn payer_pubkey(&self) -> Result<Address, SolanaError> {
        let kp = Keypair::try_from(self.payer_bytes.as_slice())
            .map_err(|_| SolanaError::VmBootFailed("invalid payer bytes in snapshot".into()))?;
        Ok(kp.pubkey())
    }
}
