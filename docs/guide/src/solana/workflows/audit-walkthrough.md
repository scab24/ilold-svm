# Solana Audit Walkthrough

This walkthrough mirrors the [Solidity audit walkthrough](../../solidity/workflows/audit-walkthrough.md) against the canonical Solana staking fixture under `tests/fixtures/solana/staking`. The program exposes five instructions — `initialize_pool`, `stake`, `unstake`, `add_rewards`, `claim_rewards` — and ships with a pre-built `bin/staking.so` so the suite runs without the Anchor toolchain.

## Starting the session

Launch ilold against the Anchor workspace:

```
ilold explore tests/fixtures/solana/staking
```

The REPL boots a LiteSVM with `bin/staking.so` and auto-selects the program:

```
ilold[staking]>
```

List the instructions and account types to orient yourself:

```
ilold[staking]> f
  initialize_pool   args=1   accounts=3   signers=1   pdas=1
  stake             args=1   accounts=4   signers=1   pdas=1
  unstake           args=1   accounts=4   signers=1   pdas=1
  add_rewards       args=1   accounts=2   signers=1   pdas=0
  claim_rewards     args=0   accounts=4   signers=1   pdas=1

ilold[staking]> v
  Pool          disc=0x4e2a...
  UserStake     disc=0x8c91...
```

This is the starting map: five entry points, two account types.

## Creating users and bootstrapping the pool

Solana sessions need accounts to drive. Mint the keypairs you need:

```
ilold[staking]> users new admin 100000000
  ✓ admin   pubkey=AdminPk…   balance=0.1 SOL

ilold[staking]> users new pool 2000000
ilold[staking]> users new alice 50000000
ilold[staking]> users new alice_stake 2000000
```

`users new` creates a keypair and airdrops it. Unmapped account names in a later `call` will be coerced to local keypairs as well, but pre-creating them makes the session deterministic.

Now initialize the pool:

```
ilold[staking]> call initialize_pool reward_rate=10 pool=pool admin=admin
  + Step 0: initialize_pool   CU 12.4k   1 account written
```

`call` runs the instruction in the VM and appends the result. The output lists CU consumed and the number of mutated accounts.

## Staking and observing state

```
ilold[staking → initialize_pool]> call stake amount=1000 pool=pool user_stake=alice_stake user=alice
  + Step 1: stake   CU 18.7k   2 accounts written
```

Check the accumulated state:

```
ilold[staking → … → stake]> state
  Pool 7XzG…ABCd
    admin           = AdminPk…
    reward_rate     = 10
    total_staked    = 1000
  UserStake 6Hj…Pq
    user            = AlicePk…
    amount          = 1000
    reward_debt     = 0
```

## Inspecting a step in detail

`step <i>` re-prints the persisted CU, logs, and decoded diffs for a specific step:

```
ilold[staking → … → stake]> step 1
  step 1   stake
    CU       18.7k
    accounts written:
      Pool 7XzG…ABCd   total_staked: 0 → 1000
      UserStake 6Hj…Pq amount: — → 1000
    logs:
      Program log: Instruction: Stake
      ...
```

## Cross-step questions

`who` resolves a query against the IDL. Use it to find which instructions touch a given account type:

```
ilold[staking → … → stake]> who Pool
  AccountType: Pool
  Instructions:
    initialize_pool   accounts: pool [init, mut]
    stake             accounts: pool [mut]
    unstake           accounts: pool [mut]
    add_rewards       accounts: pool [mut]
    claim_rewards     accounts: pool [mut]
```

`timeline <pubkey>` shows how a specific account has evolved across the session, with decoded field diffs:

```
ilold[staking → … → stake]> timeline pool
  Pool 7XzG…ABCd
    step 0  initialize_pool
      admin         = AdminPk…   (— → AdminPk…)
      reward_rate   = 10          (— → 10)
      total_staked  = 0           (— → 0)
    step 1  stake
      total_staked  = 1000        (0 → 1000)
```

## Recording findings and exporting

Capture an observation as a note and record a finding tied to the latest step:

```
ilold[staking → … → stake]> n staking does not enforce min stake amount
  ✓ Note added

ilold[staking → … → stake]> finding High "missing reentrancy guard" --rec="Apply checks-effects-interactions"
  ✓ Finding F-001 added (High)
```

Mark instructions as you go:

```
ilold[staking]> status stake reviewed
ilold[staking]> status claim_rewards finding
```

Export the deliverable:

```
ilold[staking]> export --auditor="Demo Auditor" --version="v0.1.0" --date=2026-05-09
  ✓ Exported
```

## Persisting the session

```
ilold[staking]> save my-audit --with-keypairs
  ✓ Saved to ~/.ilold/sessions/my-audit.json
  ⚠  bundle includes plaintext test keypairs — do NOT commit it

ilold[staking]> clear
  Cleared 2 step(s).

ilold[staking]> load my-audit
  ⚠  bundle contains plaintext test keypairs — do NOT commit *.json files like this
  ✓ Session loaded (2 steps)
```

`--with-keypairs` is mandatory when the audit relies on deterministic PDAs (which depend on signer pubkeys): without it, `load` regenerates fresh keypairs and PDAs come back at different addresses.

## Parallel to the Solidity walkthrough

The flow is structurally identical to the Solidity one:

1. `f` / `v` to map the surface (instructions and account types instead of functions and state variables).
2. `users new` + `call` to push the VM forward (vs. `c <func>` on a parsed CFG).
3. `state`, `step`, `timeline` to inspect what changed.
4. `who` to navigate cross-instruction relationships (vs. cross-function in Solidity).
5. `finding`, `note`, `status` to record observations.
6. `export`, `save`, `load` to ship the deliverable and resume later.

What is **not** available yet on Solana: `slice` and `trace`. Both require the Anchor handler AST and are tracked in [Roadmap: Solana Phase 2](../../roadmap/solana.md).

## Related pages

- [Session](../repl/session.md), [Programs and IDL](../repl/programs.md), [Solana runtime](../repl/runtime.md), [Analysis](../repl/analysis.md), [Findings](../repl/findings.md)
- [Scenarios and forks](./scenarios.md)
- [Solana: Limitations](../limitations.md)
