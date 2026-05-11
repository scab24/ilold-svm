# Getting Started

## Installation

Clone the repository and build from source:

```
git clone https://github.com/scab24/ilold.git
cd ilold
cargo build --release
```

The binary is at `target/release/ilold`. The four subcommands are `analyze`, `context`, `serve`, and `explore` (see `crates/ilold-cli/src/main.rs`).

## Backend detection

`serve` and `explore` auto-detect the backend from the path:

- A directory or file containing `.sol` sources is treated as a Solidity project.
- A directory containing `Anchor.toml` (with `idls/<program>.json`) is treated as a Solana project.

`analyze` and `context` are Solidity-only.

## Running against a Solidity project

```
cargo run -- explore tests/fixtures/staking.sol
```

ilold parses the files, builds the model and per-function CFGs, and drops the auditor into the REPL:

```
  ╭──────────────────────────────────────────╮
  │ ilold explore — Staking                  │
  │ 8 functions | Type ? for help            │
  │ Web UI: http://localhost:52431           │
  ╰──────────────────────────────────────────╯

ilold[Staking]>
```

The port defaults to `0` (auto-assigned) for `explore` and `8080` for `serve`. Override with `--port`.

## Running against a Solana project

```
cargo run -- explore tests/fixtures/solana/staking
```

ilold loads the IDL under `idls/<program>.json`, boots a LiteSVM with the program binary from `target/deploy/<program>.so` (or `bin/<program>.so`), and opens the REPL:

```
ilold[staking]>
```

Without a compiled `.so` the REPL still starts and IDL navigation (`f`, `i`, `pda`, `vars`) works, but commands that drive the VM (`call`, `state`, `inspect`) fail until the program is built.

## First session

A typical first exploration of the Solidity staking contract:

```
ilold[Staking]> f

  [P] deposit         writes state, external calls
  [P] withdraw        writes state, external calls
  [P] claimRewards    writes state, external calls
  [R] setRewardRate   writes state
  [R] pause           writes state
  [R] unpause         writes state
  [P] rewardPerToken  view
  [P] earned          view
```

```
ilold[Staking]> c deposit

  + Step 0: deposit [P] external
    State writes:
      · balances[msg.sender]
      · lastUpdateTime
      · rewardPerTokenStored
      · rewards[account]
      · totalStaked
      · userRewardPerTokenPaid[account]
    Sequence: deposit
```

```
ilold[Staking → deposit]> s

  ════════════════════════════════════════════[ STATE ]═════════════════════════════════════════════
  balances[msg.sender]
    += amount (step 0:15, deposit)
  lastUpdateTime
    = block.timestamp (step 0:8, deposit)
  rewardPerTokenStored
    = rewardPerToken() (step 0:7, deposit)
  rewards[account]
    = earned(account) (step 0:11, deposit)
  totalStaked
    += amount (step 0:16, deposit)
  userRewardPerTokenPaid[account]
    = rewardPerTokenStored (step 0:12, deposit)
```

The full audit flow (`who`, `tr`, `sl`, `tl`, scenarios, findings, export) is covered in [Solidity: Audit walkthrough](./solidity/workflows/audit-walkthrough.md). For the Solana equivalent (`users new`, `call <ix>`, `state`, `step`, `timeline <pubkey>`) see [Solana: Audit walkthrough](./solana/workflows/audit-walkthrough.md).

## Inline help

Type `?` at the prompt for the full command reference. Append `?` to any command for its usage:

```
ilold[Staking]> sl?
  slice <func> <var> [--backward]  Dataflow slice. Example: sl deposit totalStaked --backward
```

On Solana, appending `?` renders the structured help block for the command, including syntax, flags, examples, return shape, and related commands.
