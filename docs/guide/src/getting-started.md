# Getting Started

## Installation

Clone the repository and build from source:

```
git clone https://github.com/scab24/ilold.git
cd ilold
cargo build --release
```

The binary is at `target/release/ilold`. The three subcommands are `serve`, `explore`, and `mcp` (see `crates/ilold-cli/src/main.rs`).

## Running against a Solana project

```
cargo run -- explore tests/fixtures/solana/staking
```

ilold detects the project as Solana via `Anchor.toml`, loads the IDL under `idls/<program>.json`, boots a LiteSVM with the program binary from `target/deploy/<program>.so` (or `bin/<program>.so`), and opens the REPL:

```
ilold[staking]>
```

The port defaults to `0` (auto-assigned) for `explore` and `8080` for `serve`. Override with `--port`.

Without a compiled `.so` the REPL still starts and IDL navigation (`f`, `i`, `pda`, `vars`) works, but commands that drive the VM (`call`, `state`, `inspect`) fail until the program is built.

## First session

A typical first exploration of the staking fixture:

```
ilold[staking]> f

  initialize_pool        0a 4acc 1sig 1pda
  stake                  1a 4acc 1sig 1pda
  unstake                1a 4acc 1sig 1pda
  claim_rewards          0a 4acc 1sig 1pda
  set_reward_rate        1a 2acc 1sig 0pda
```

```
ilold[staking]> users new alice
  ✓ alice created with 10 SOL
```

```
ilold[staking]> c stake amount=1000 pool=pool user_stake=alice_stake user=alice

  + Step 0: stake
    accounts: pool, alice_stake (pda), alice (signer)
    compute_units: 4823
    diffs: alice_stake.amount = 1000
```

```
ilold[staking]> state

  alice_stake (UserStake)
    amount = 1000
    last_update_ts = 1715789432
  pool (Pool)
    total_staked = 1000
```

The full audit flow (`users new`, `call <ix>`, `state`, `step`, `timeline <pubkey>`, scenarios, findings, export) is covered in [Audit walkthrough](./solana/workflows/audit-walkthrough.md).

## Inline help

Type `?` at the prompt for the full command reference. Append `?` to any command for its structured usage block (syntax, flags, examples, return shape, related commands):

```
ilold[staking]> call?
  ... structured help block ...
```
