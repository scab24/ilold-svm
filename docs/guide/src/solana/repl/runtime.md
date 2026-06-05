# Solana Runtime Commands

These commands operate directly on the LiteSVM owned by the active scenario.

## users

`users`: list keypairs in the active scenario.

`users new <name> [lamports]`: create a keypair and airdrop it. Default airdrop is `10_000_000_000` lamports (10 SOL).

```
ilold[staking]> users new alice
  ✓ user alice created at 8H7…Pq with 10000000000 lamports

ilold[staking]> users new bob 5000000000
  ✓ user bob created at 6Yg…Tx with 5000000000 lamports

ilold[staking]> users
  [U] alice 8H7…Pq 10000000000 lamports
  [U] bob 6Yg…Tx 5000000000 lamports
```

**Returns:** `UserList { users: [{ name, pubkey, lamports }] }` on listing, `UserCreated { name, pubkey, lamports }` on `users new`.

## airdrop

`airdrop <user> <lamports>` (alias: `air`)

Tops up an existing keypair with extra lamports.

```
ilold[staking]> airdrop alice 1000000000
  ✓ alice now 11000000000 lamports 8H7…Pq
```

**Returns:** `Airdropped { name, pubkey, total_lamports }`.

## time-warp

`tw <delta_seconds>` or `time-warp <delta_seconds>`

Advances (or rewinds) the `Clock` sysvar so vesting / reward / lockup logic can be exercised. Positive deltas move forward, negative deltas move backward.

```
ilold[staking]> tw 86400
  ✓ clock now ts=1714147200 slot=12345

ilold[staking]> tw -3600
  ✓ clock now ts=1714143600 slot=12345
```

**Returns:** `TimeWarped { unix_timestamp, slot }`.

`time-warp` is a global side effect on the `Clock` sysvar and is **not** undone by `back`. It is the auditor's responsibility to reset the clock manually if a later test expects a specific timestamp. Negative deltas adjust `unix_timestamp` linearly but do not move the `slot` counter backwards.

## pda

`pda <ix>`

Lists the PDAs declared by an instruction (Anchor seeds plus bump). Read directly from the IDL, no VM execution required.

```
ilold[staking]> pda stake
  [PDA] user_stake seeds=["user-stake", user] program=self
```

**Returns:** `PdaList { instruction, pdas: [{ account_name, seeds, program }] }`.

## inspect

`inspect <pubkey>` (alias: `acc`)

Reads an account from the VM and decodes it via the Anchor discriminator. The pubkey can be a named keypair, a named PDA from a previous step, or a raw base58 string.

```
ilold[staking]> inspect alice
  8H7…Pq owner=11111111111111111111111111111111 lamports=10000000000 data_len=0

ilold[staking]> inspect 6Yg7...
  6Yg7…ABCd owner=StakingProgram… lamports=2039280 data_len=72
    {
      "admin": "AdminPubkey…",
      "reward_rate": 10,
      "total_staked": 1000,
      "last_update_ts": 1714060800
    }
```

**Returns:** `AccountInspected { pubkey, owner, lamports, data_len, decoded }`. `decoded` is `Some(Value)` when the Anchor discriminator matches a known account type, otherwise `None`.

## Notes

- All runtime commands are scoped to the **active scenario**. Forks own their own VM and their own keypair set; switching scenarios swaps the runtime.
- `inspect` is the easiest way to confirm what `state` and `timeline` will surface for a given account.
