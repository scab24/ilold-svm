# Session Commands

Session commands drive the active scenario: append a call, rewind, inspect what changed. Every state-changing command updates the LiteSVM as a side effect; `back` and `clear` rewind the VM to the corresponding pre-step snapshot.

## call

`c <ix> [arg=val ...] [account=user_or_pubkey ...]` or `call <ix> {json}` (alias: `c`)

Runs an Anchor instruction against the VM and appends the result to the active scenario. Two payload forms are supported:

- **Concise key=value form**: positional `arg=value` and `account_field=user_name` tokens. Unmapped names become local keypairs. Signers are auto-resolved from the IDL.
- **JSON form**: `{"args": {...}, "accounts": {...}, "signers": [...]}` for full control.

Flags:

| Flag | Description |
| --- | --- |
| `--signer=a,b` | Add signers on top of the IDL defaults |
| `--no-signer=name` | Remove a default signer (for negative cases) |

```
ilold[staking]> c initialize_pool reward_rate=10 pool=pool admin=admin
  ✓ step 0 [ok]: initialize_pool (12400 CU, 1 diffs)
```

```
ilold[staking → initialize_pool]> c stake amount=1000 pool=pool user_stake=alice_stake user=alice
  ✓ step 1 [ok]: stake (18700 CU, 2 diffs)
```

```
ilold[staking]> c stake {"args":{"amount":1000},"accounts":{"pool":"pool","user_stake":"alice_stake","user":"alice"}}
```

When the VM rejects the call (Anchor constraint, custom `require!`, etc.) no step is appended and the CLI prints the error inline:

```
ilold[staking]> c stake amount=0 pool=pool user_stake=alice_stake user=alice
  ✗ FAILED: stake (4200 CU, not recorded)
    error: AnchorError: Amount must be > 0
```

**Returns:** `StepAdded { step_index, instruction, logs_excerpt, account_diffs_count, compute_units, error }` on success, `CallFailed { instruction, logs_excerpt, compute_units, error }` when the VM rejects.

See also: [`info`](./programs.md#info), [`pda`](./runtime.md#pda), [`state`](#state), [`step`](#step), [`back`](#back).

## back

`b` or `back`

Removes the last step from the active scenario and rewinds the VM to the pre-call snapshot of that step. Re-issuing the same `call` after `back` produces fresh CU and diffs.

```
ilold[staking → initialize_pool → stake]> b
  ✓ step undone (1 remaining)
```

**Returns:** `StepRemoved { remaining }`.

`time-warp` is a global side effect on the `Clock` sysvar and is **not** undone by `back`. Reset the clock manually if a test relies on a specific timestamp.

## clear

`cl` or `clear`

Drops every step in the active scenario and rewinds the VM to the genesis snapshot of that scenario.

```
ilold[staking → initialize_pool → stake]> cl
  ✓ session cleared
```

**Returns:** `Cleared`.

## state

`state`

Decoded view of every account mutated during the active scenario. Each entry shows the pubkey, the owning program, and the decoded fields from the latest step.

```
ilold[staking → initialize_pool → stake]> state
  [A] pool (2039280 lamports) 7XzG…ABCd
      admin          AdminPubkey…
      reward_rate    10
      total_staked   1000
      last_update_ts 1714060800
  [A] alice_stake (1559040 lamports) 6Hj…Pq
      user           AlicePubkey…
      amount         1000
      reward_debt    0
```

**Returns:** `StateView { accounts: [AccountSummary { pubkey, label, lamports, decoded }] }`. `decoded` is the Anchor-decoded JSON snapshot, `None` for accounts whose discriminator does not match a known type.

## session

`s` or `session`

Prints the active scenario summary: ordered steps, findings, notes, and the current scenario name.

```
ilold[staking → initialize_pool → stake]> s
  program=staking scenario=main steps=2 findings=0
    0. initialize_pool
    1. stake
```

**Returns:** `SessionView { program, scenario, steps, findings_count }`.

## step

`st <index>` or `step <index>` (no-space shortcut: `st0`, `step1`)

Re-inspects a specific step of the active scenario, printing the persisted CU, logs, and decoded account diffs.

```
ilold[staking → initialize_pool → stake]> step 1
  · step 1 · stake
    compute units: 18700
    logs: (4 lines)
    diffs (2):
      pool (Pool)
        total_staked  0 → 1000
      alice_stake (UserStake)
        amount        — → 1000
```

**Returns:** `StepDetail { step_index, instruction, runtime_trace, diff_summary }`. `runtime_trace` is a JSON blob carrying `compute_units`, `logs`, and (when present) `error`; `diff_summary` is a list of `{ address, name, lamports_delta, data_changed, decoded_before, decoded_after }`.

## Notes

- `seq` / `sequence` is currently aliased to `session`; a cross-step narrative engine is tracked in the [Roadmap](../../roadmap/solana.md).
- `call` is the only command that drives the VM forward; everything else inspects or rewinds.
