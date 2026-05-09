# Solana Support

Ilold also analyzes Solana programs (Anchor IDL based) through the same REPL,
with a different execution backend. This page explains what works, what does
not, and how the auditor workflow maps from Solidity to Solana.

## Loading a Solana program

Point ilold at a directory that contains `Anchor.toml`, `idls/<program>.json`
and (optionally) a compiled `target/deploy/<program>.so`:

```
ilold serve tests/fixtures/solana/staking
```

Without the `.so` the REPL still loads — IDL navigation works — but anything
that requires VM execution (`call`, `state`, `inspect`) fails until the
program is built.

## Mental model: how Solana differs from Solidity

| Concept | Solidity | Solana (Anchor) |
| --- | --- | --- |
| Entry point | function on a contract | instruction on a program |
| Persistent state | contract state variables | accounts owned by the program |
| Caller identity | `msg.sender` | signers passed by the client |
| Side-effect surface | storage writes / events | account mutations + logs |
| Execution | symbolic (CFG + paths) | concrete (LiteSVM with real bytecode) |

Because the Solana side runs the real BPF program, every Call needs the
auditor to provide accounts (often new keypairs created with `users new`)
and signers. PDAs are derived on demand, not declared up front.

## Quickstart

```text
ilold[staking]> users new admin 100000000
ilold[staking]> users new pool 2000000
ilold[staking]> users new alice 50000000
ilold[staking]> users new alice_stake 2000000
ilold[staking]> call initialize_pool reward_rate=10 pool=pool admin=admin
ilold[staking → initialize_pool]> call stake amount=1000 pool=pool user_stake=alice_stake user=alice
ilold[staking → initialize_pool → stake]> state
ilold[staking → initialize_pool → stake]> step 1
ilold[staking → initialize_pool → stake]> who Pool
ilold[staking → initialize_pool → stake]> timeline <pool-pubkey>
ilold[staking → initialize_pool → stake]> finding High "missing reentrancy guard"
ilold[staking → initialize_pool → stake]> export
```

`call` accepts two forms: a concise key-value form (`arg=value`,
`account_field=user_name`) where unmapped names are turned into local
keypairs, or a fully explicit JSON payload `call <ix> {"args":...,
"accounts":...,"signers":...}`.

## Command parity

Everything documented under [Session](./commands/session.md), [Contract](./commands/contract.md),
[Findings](./commands/findings.md) and [Workspace](./commands/workspace.md)
works against Solana with the same syntax. The Analysis surface is partial:

| Command | Status | Notes |
| --- | --- | --- |
| `info <ix>` | works | args + account flags + discriminator |
| `who <account_type>` | works | maps `snake_case` field names to PascalCase types |
| `timeline <pubkey>` | works | cross-step mutation history with before/after decoded |
| `step <index>` | works | re-prints CU, logs, account diffs |
| `findings` / `export` | works | Markdown report includes scenario, sequence, findings |
| `slice` | not implemented | requires Anchor handler AST — Phase 2 |
| `trace` | not implemented | requires per-instruction CFG — Phase 2 |
| `sequence` | aliased to `session` | full narrative with CPI dependencies needs CFG |

## Solana-only commands

| Command | Use |
| --- | --- |
| `users new <name> [lamports]` | create a keypair and airdrop SOL into it |
| `airdrop <name> <lamports>` | top up an existing keypair |
| `time-warp <delta_seconds>` | advance the `Clock` sysvar (positive forward, negative back) |
| `pda <ix>` | list PDAs declared by the instruction in the IDL |
| `inspect <pubkey>` | decode an account by Anchor discriminator |

## Scenarios and VM rewind

Scenarios (`scenario new`, `scenario fork`, `scenario switch`) keep an
independent VM per branch. `back` and `clear` rewind the VM to the
pre-step snapshot, so re-issuing the same Call after `back` produces
fresh CU and diffs (instead of replaying stale state). `time-warp` is a
global side-effect on the Clock and is not undone by `back` — it is the
auditor's responsibility to reset the clock if needed.

## Save and load

`save <name>` writes the entire scenario tree (steps, runtime traces,
findings, fork origins, original Call inputs) to
`~/.ilold/sessions/<name>.json`. `load <name>` reboots a fresh VM per
scenario, re-airdrops the in-memory users, and replays each Call from
the persisted payload. The reconstructed VM state matches the saved
snapshot for typical Anchor flows; programs with non-deterministic
behaviour or balances above the default replay cap may diverge.

## Known limitations

- No static control-flow graph or path-tree analysis (Solana programs
  are bytecode at this point, not parsed Rust).
- `who` is heuristic: it matches account field names against IDL account
  types via snake_case → PascalCase, so unconventional naming will miss.
- `time-warp` advances `unix_timestamp` linearly but does not move the
  slot counter backwards on negative deltas.
- LoadSession is best-effort for legacy saves without `call_payload`
  (timeline restores, VM stays at genesis).
