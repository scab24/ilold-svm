# Programs and IDL Commands

These commands inspect the static surface of the active program: instruction list, instruction detail, account types, and project-level navigation.

## programs / contracts

`ct` or `programs` (aliases: `contracts`, `progs`)

Lists every program detected in the workspace. Multi-program Anchor workspaces (e.g. `tests/fixtures/solana/cpi`) show one entry per program.

```
ilold[staking]> ct
  staking          ← current
  reward_oracle
```

## use

`use <program>`

Switches the active program. Subsequent commands target the new program; the prompt label updates.

```
ilold[staking]> use reward_oracle
  ✓ now using reward_oracle
```

## functions

`f` or `funcs` (alias: `functions`)

Lists the instructions exposed by the active program, with arg / account counts and signer names. PDA-bearing instructions are tagged `[PDA]`.

```
ilold[staking]> f
  [PDA] initialize_pool (args:1 accounts:3) signers: admin
  [PDA] stake (args:1 accounts:4) signers: user
  [PDA] unstake (args:1 accounts:4) signers: user
  [ix]  add_rewards (args:1 accounts:2) signers: admin
  [PDA] claim_rewards (args:0 accounts:4) signers: user
```

**Returns:** `InstructionList { items: [InstructionEntry { name, args_count, accounts_count, has_pdas, signers }] }`.

## funcs-all

`fa` or `funcs-all`

Currently identical to `funcs`: both dispatch to the same `Funcs` command and return `InstructionList`. The admin-gating and coupling hints surface separately via [`info`](#info) (`admin_gated` flag) and [`coupling`](./analysis.md#coupling).

```
ilold[staking]> fa
  [PDA] initialize_pool (args:1 accounts:3) signers: admin
  [PDA] stake (args:1 accounts:4) signers: user
  ...
```

**Returns:** `InstructionList { items: [...] }`: same shape as `funcs`.

## info

`i <ix>` or `info <ix>`

Full detail of an instruction: discriminator, typed args, accounts with their `signer` / `writable` / `optional` flags and kind (`system`, `sysvar`, `program`, `pda`, `other`), declared PDAs with their seeds, and the admin-gating heuristic.

```
ilold[staking]> i stake
  instruction stake
  discriminator 0xa1b2c3...

  args (1)
    · amount u64

  accounts (4)
    · pool         other  writable
    · user_stake   pda    writable
    · user         other  signer writable
    · system_program  program  const 11111111111111111111111111111111

  pdas (1)
    · user_stake seeds=["user-stake", user] program=self

  admin_gated false
```

**Returns:** `IxInfo { ix: IxView, admin_gated: bool }`. `IxView` carries `name`, `discriminator_hex`, `args`, `accounts`, and per-account `pda` metadata.

See also: [`funcs-all`](#funcs-all), [`pda`](./runtime.md#pda), [`who`](./analysis.md#who), [`call`](./session.md#call).

## vars

`v` or `vars`

Lists the account types declared in the IDL, with their Anchor discriminator and field layout. Solana does not split `vars` / `vars-all`: both alias to the same `Vars` command.

```
ilold[staking]> v
  [T] Pool 0x4e2a...
    · admin           Pubkey
    · reward_rate     u64
    · total_staked    u64
    · last_update_ts  i64
  [T] UserStake 0x8c91...
    · user            Pubkey
    · amount          u64
    · reward_debt     u64
```

**Returns:** `AccountTypes { accounts: [AccountView { name, discriminator_hex, fields: [FieldView { name, ty }] }] }`.

## vars-all

`va` or `vars-all`

Aliased to `vars` at the dispatcher level: same command, same output.

## Notes

- The Solidity counterpart of these commands is documented under [Contract](../../solidity/repl/contract.md). The shapes line up so an auditor moving between backends sees the same structure.
- `use` clears the displayed step list for the previous program. The underlying scenario state for that program is preserved and reappears when switching back.
