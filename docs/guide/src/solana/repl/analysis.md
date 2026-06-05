# Analysis Commands

The Solana analysis surface is partial: there is no static CFG yet, so `slice`, `trace`, and the dedicated `sequence` narrative are not implemented (`sequence` is aliased to `session`). The following commands are available today:

| Command | Status | What it reads |
| --- | --- | --- |
| `who <query>` | works | IDL: account types, instructions, struct fields |
| `timeline <pubkey>` | works | account diffs accumulated by the active scenario |
| `coupling` | works | IDL + accounts metadata |
| `coverage` | works | runtime metrics over the active scenario |
| `slice` / `trace` | not implemented | requires the Anchor handler AST (Phase 2) |

## who

`who <AccountType | ix_name | field_name>`

Resolves a query against the IDL. The same command answers three different questions depending on the input.

**Account type**: list instructions that reference accounts of that type. The lookup is case-insensitive with a snake_case → PascalCase fallback, so `who pool` and `who Pool` both work.

```
ilold[staking]> who Pool
  · 'Pool' (account type)
    fields: admin: Pubkey, reward_rate: u64, total_staked: u64, last_update_ts: i64

  Referenced by 5 instructions:

    · initialize_pool (as pool) writable
        args: reward_rate: u64
    · stake (as pool) writable
        args: amount: u64
    · unstake (as pool) writable
        args: amount: u64
    · add_rewards (as pool) writable
        args: amount: u64
    · claim_rewards (as pool) writable
        args: (none)
```

**Instruction**: list accounts the instruction touches, plus its args and discriminator:

```
ilold[staking]> who claim_rewards
  · 'claim_rewards' (instruction)
    args: (none)
    discriminator 0xa1b2c3...

  Touches 4 accounts:

    · pool (Pool) writable
    · user_stake (UserStake) writable
    · user signer
    · reward_vault writable
```

**Field**: identify the owning type and the instructions that write the owner account (heuristic without source-level analysis we cannot tell which writer actually mutates this field).

```
ilold[staking]> who total_staked
  · 'total_staked' (field of Pool, type u64)
    Pool struct: admin: Pubkey, reward_rate: u64, total_staked: u64, last_update_ts: i64

  Heuristic: the following instructions write the owner account.
  Without source-level analysis we cannot tell which one(s)
  actually mutate this field; cross-check with `step <idx>`.

    · stake (as pool) writable
    · unstake (as pool) writable
```

**Returns:** `WhoList { account_type, instructions, query_kind, field_owner, field_type, owner_fields, ix_args, ix_discriminator_hex, ix_accounts }`. `query_kind` is one of `AccountType`, `Field`, `Instruction`, `NotFound`.

See also: [`info`](./programs.md#info), [`funcs`](./programs.md#functions), [`vars`](./programs.md#vars), [`coupling`](#coupling).

## timeline

`tl <pubkey>` or `timeline <pubkey>`

Shows the cross-step mutation history of an account, decoded. The pubkey can be a named keypair, a named PDA, or a raw base58 string.

```
ilold[staking → initialize_pool → stake]> tl pool
  · timeline for pool (7XzG…ABCd)
    · #0 initialize_pool (main) data
        {"admin":"AdminPubkey…","reward_rate":10,"total_staked":0,"last_update_ts":0}
    · #1 stake (main) data
        {"admin":"AdminPubkey…","reward_rate":10,"total_staked":1000,"last_update_ts":1714060800}
```

**Returns:** `TimelineView { pubkey, label, entries: [{ step_index, instruction, scenario, lamports_delta, data_changed, before_decoded, after_decoded }] }`.

## coupling

`cp` or `coupling`

Lists instruction pairs that share a writable account. Surfaces instructions that may interfere through shared writable state (ProgramView heuristic).

```
ilold[staking]> coupling
  · stake  ↔  unstake          [pool, user_stake]
  · stake  ↔  claim_rewards    [pool, user_stake]
  · add_rewards  ↔  claim_rewards    [pool]
```

**Returns:** `CouplingList { pairs: [{ a, b, shared_writable: [..] }] }`.

## coverage

`cov` or `coverage`

Aggregated runtime metrics over the active scenario: calls, failures, CU stats, CPI edges (RuntimeOverlay).

```
ilold[staking → initialize_pool → stake]> cov
  Coverage for program staking (scenario main)

  Instruction        Calls Failed CU avg CU max CPIs
  initialize_pool    1     0      12400  12400  0
  stake              1     0      18700  18700  0

  Total: 2 calls, 0 failed
```

**Returns:** `Coverage { overlay: { program, scenario, calls_per_ix, failed_per_ix, cu_stats_per_ix, cpi_edges } }`.

Coverage is the closest current surrogate for "have I exercised every instruction?": it makes it easy to spot instructions never called, instructions that always fail, and programs reached only through CPI.

## Notes

- See [Limitations](../limitations.md) for the static-analysis gap (no CFG → no `slice` / `trace` yet).
- `who` works against account types (and fields with a snake_case → PascalCase heuristic). `timeline` works against decoded account pubkeys.
