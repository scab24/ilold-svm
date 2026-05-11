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

#[derive(Clone)]
pub struct StateSnapshot {
    pub accounts: Vec<(Address, AccountSharedData)>,
    pub clock: Clock,
}

impl VmHost {
    pub fn snapshot_state(&self) -> StateSnapshot {
        let accounts = self
            .svm()
            .accounts_db()
            .inner
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        StateSnapshot {
            accounts,
            clock: self.svm().get_sysvar::<Clock>(),
        }
    }

    pub fn restore_state(&mut self, snap: StateSnapshot) -> Result<(), SolanaError> {
        let live: std::collections::HashSet<Address> =
            self.svm().accounts_db().inner.keys().copied().collect();
        let snap_keys: std::collections::HashSet<Address> =
            snap.accounts.iter().map(|(k, _)| *k).collect();

        let to_drop: Vec<Address> = live.difference(&snap_keys).copied().collect();
        for pk in to_drop {
            let empty: solana_account::Account = solana_account::Account::default();
            self.svm_mut().set_account(pk, empty).map_err(|e| {
                SolanaError::VmOperationFailed(format!("drop account {pk}: {e:?}"))
            })?;
        }

        let upgradeable = solana_sdk_ids::bpf_loader_upgradeable::id();
        const PROGRAMDATA_DISCRIMINATOR_BYTE: u8 = 3;
        use solana_account::ReadableAccount;
        let (programdata, other): (Vec<_>, Vec<_>) =
            snap.accounts.iter().cloned().partition(|(_, acc)| {
                acc.owner() == &upgradeable
                    && acc.data().first() == Some(&PROGRAMDATA_DISCRIMINATOR_BYTE)
            });
        for (pk, acc) in programdata {
            self.svm_mut().set_account(pk, acc.into()).map_err(|e| {
                SolanaError::VmOperationFailed(format!("restore programdata {pk}: {e:?}"))
            })?;
        }
        for (pk, acc) in other {
            self.svm_mut().set_account(pk, acc.into()).map_err(|e| {
                SolanaError::VmOperationFailed(format!("restore account {pk}: {e:?}"))
            })?;
        }
        self.svm_mut().set_sysvar(&snap.clock);
        Ok(())
    }
}
