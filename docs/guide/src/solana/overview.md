# Solana Backend Overview

The Solana backend runs Anchor programs against a LiteSVM-backed engine. Every `call` runs in the VM, so the auditor sees real compute units, real logs, and real account state. Static control-flow analysis is not available yet; anything that requires it (`slice`, `trace`, `sequence` narrative) is deferred to Phase 2 (see [Roadmap](../roadmap/solana.md)).

Two CLI entry points cover Solana: `ilold explore <project>` (REPL + API) and `ilold serve <project>` (API only). Both auto-detect the Solana backend when the path resolves to an Anchor workspace.

## Project layout the loader expects

```
<root>/
  Anchor.toml
  idls/
    <program>.json          # Anchor IDL (required)
  target/deploy/<program>.so  # compiled program (or bin/<program>.so)
```

`crates/ilold-solana-core/src/ingest` resolves these paths. Without the `.so`, IDL navigation (`f`, `i`, `pda`, `vars`, `who`) still works; everything that drives the VM (`call`, `state`, `inspect`, `timeline`) fails until the program is compiled.

The committed fixtures live under `tests/fixtures/solana/staking` (single program) and `tests/fixtures/solana/cpi` (two programs that talk to each other through CPI). Both ship pre-built `bin/<program>.so` binaries so the suite runs without the Anchor toolchain.

## Solidity vs Solana mental model

| Concept | Solidity | Solana |
| --- | --- | --- |
| Entry point | function on a contract | instruction on a program |
| Persistent state | contract state variables | accounts owned by the program |
| Caller identity | `msg.sender` (implicit) | signers passed by the client |
| `who <X>` | reads/writes of a state variable (CFG-based) | instructions that touch an account type (IDL heuristic) |
| `timeline <X>` | mutation history of a state variable | mutation history of an account pubkey, decoded |
| `step <i>` | re-renders the persisted flow tree | re-prints CU, logs, account diffs |
| `slice` / `trace` | full CFG-based analysis | not implemented (Phase 2) |
| `sequence` | narrative with cross-step dependencies | aliased to `session` (no narrative engine yet) |
| Execution | symbolic (CFG + paths) | concrete (in-process LiteSVM execution) |
| `back` | drops the step from the timeline | drops the step AND rewinds the VM to the pre-call snapshot |
| `save` / `load` | step list + persisted paths | step list + replay-driven VM reconstruction |

## REPL command groups

The REPL command surface mirrors the Solidity one with backend-specific extensions. Each group has its own page:

- [Session](./repl/session.md): `c/call`, `b/back`, `cl/clear`, `s/session`, `state`, `st/step`.
- [Programs and IDL](./repl/programs.md): `ct/programs`, `use`, `f/funcs`, `fa/funcs-all`, `i/info`, `v/vars`, `va/vars-all`.
- [Solana runtime](./repl/runtime.md): `users`, `airdrop`, `tw/time-warp`, `pda`, `inspect`.
- [Analysis](./repl/analysis.md): `who`, `tl/timeline`, `cp/coupling`, `cov/coverage`.
- [Findings](./repl/findings.md): `fi/finding`, `n/note`, `status`, `fl/findings`, `ex/export`.
- [Scenarios](./repl/scenarios.md): `sc/scenario` (`new`, `list`, `switch`, `fork`, `delete`).
- [Workspace](./repl/workspace.md): `save`, `load`, `browser`.
- [Help and control](./repl/help.md): `?/help`, `<cmd>?`, `q/quit/exit`, `seq` (aliased to `session`).

## Workflows

- [Audit walkthrough](./workflows/audit-walkthrough.md): staking program end-to-end, paralleling the Solidity walkthrough.
- [Scenarios and forks](./workflows/scenarios.md): branching VMs, rewinding the clock, persisting bundles.
