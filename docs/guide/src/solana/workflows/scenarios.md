# Scenarios and Forks

Scenarios are the auditor's tool for asking "what if?" against a real VM without losing the current line of reasoning. This page walks through a forking session against `tests/fixtures/solana/staking` and shows how `back`, `clear`, `fork`, and `time-warp` interact.

## Setup

Boot the REPL with the staking fixture and set up the happy path:

```
ilold[staking]> users new admin 100000000
ilold[staking]> users new pool 2000000
ilold[staking]> users new alice 50000000
ilold[staking]> users new alice_stake 2000000

ilold[staking]> call initialize_pool reward_rate=10 pool=pool admin=admin
  + Step 0: initialize_pool   CU 12.4k

ilold[staking]> call stake amount=1000 pool=pool user_stake=alice_stake user=alice
  + Step 1: stake   CU 18.7k

ilold[staking → initialize_pool → stake]>
```

## Branching with `scenario fork`

Now diverge: what happens if a malicious caller tries `unstake` for more than the staked amount?

```
ilold[staking → initialize_pool → stake]> sc fork over-unstake 2
  ✓ Forked 'main' → 'over-unstake' at step 2
ilold[staking/over-unstake → initialize_pool → stake]>
```

The fork inherits steps `0..2` from `main` and rewinds the VM to the snapshot taken just before step 2 would have executed. The new scenario is active. The `main` scenario keeps its original state untouched.

Try the attack on the fork:

```
ilold[staking/over-unstake → initialize_pool → stake]> call unstake amount=999999 pool=pool user_stake=alice_stake user=alice
  ✗ Step rejected   CU 7.2k
    error: Custom { code: 6001 }   "InsufficientStake"
```

`CallFailed` is recorded as a step; the VM stays at the pre-call snapshot.

## Rewinding with `back`

If you want to retry, `back` drops the failed step **and** rewinds the VM:

```
ilold[staking/over-unstake → … → stake]> b
  - Step removed. 2 remaining.

ilold[staking/over-unstake → … → stake]> call unstake amount=500 pool=pool user_stake=alice_stake user=alice
  + Step 2: unstake   CU 16.1k
```

The replayed `call` produces fresh CU and diffs because the VM was rewound, not a cached replay.

## Switching back to `main`

```
ilold[staking/over-unstake → … → unstake]> sc switch main
ilold[staking → initialize_pool → stake]>
```

`main` still has its original two steps; the fork's state never leaked. Each scenario carries its own VM, signers, and PDAs.

## Time-warping vesting / reward logic

`time-warp` advances the `Clock` sysvar. It is **scenario-local but step-independent**: `back` does not rewind the clock. Use it to exercise reward accrual:

```
ilold[staking → … → stake]> tw 86400
  ✓ Clock unix_timestamp += 86400

ilold[staking → … → stake]> call claim_rewards pool=pool user_stake=alice_stake user=alice
  + Step 2: claim_rewards   CU 22.3k
```

If a later step needs a different clock, undo the offset manually with `tw -86400`.

## Persisting the scenario tree

`save` and `load` cover the whole scenario tree, not just the active one. Use `--with-keypairs` whenever a PDA depends on a signer pubkey (most Anchor programs):

```
ilold[staking]> save staking-attack --with-keypairs
  ✓ Saved to ~/.ilold/sessions/staking-attack.json
  ⚠  bundle includes plaintext test keypairs — do NOT commit it

ilold[staking]> clear
ilold[staking]> load staking-attack
  ⚠  bundle contains plaintext test keypairs — do NOT commit *.json files like this
  ✓ Session loaded (2 steps)
```

`load` reboots a fresh VM per scenario, re-airdrops the users, and replays each call. The final state matches the saved snapshot for typical Anchor flows. Non-deterministic programs may diverge, see [Solana: Limitations](../limitations.md).

## Practical patterns

| Pattern | Commands |
| --- | --- |
| "Keep the original timeline, try a divergent path" | `sc fork <name> <step>`, then continue with `call` |
| "Retry the last failed call" | `b`, edit args, `call` again |
| "Start over without losing other scenarios" | `cl`  on the active scenario, then re-issue calls |
| "Reproduce later, same addresses" | `save <name> --with-keypairs`, `load <name>` |
| "Reproduce later, fresh randomness" | `save <name>`, `load <name>` |

## Related pages

- [Scenarios](../repl/scenarios.md): command reference.
- [Workspace](../repl/workspace.md): save/load details.
- [Solana: Limitations](../limitations.md): what survives `save`/`load` and what doesn't.
