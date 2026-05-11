# What ilold does

ilold is a smart-contract audit explorer. It loads a project, builds an internal model, and exposes that model through an interactive REPL. The auditor adds entry-point calls to a **session**, the tool tracks accumulated effects, and analysis commands answer questions about the code without requiring a separate run.

## Two backends, one shell

| Backend | Input | Execution model | What you get |
| --- | --- | --- | --- |
| Solidity | `.sol` sources (file or directory) | Symbolic (parser, CFG, path tree, slicer) | Function narratives, execution trees with modifier inlining, backward/forward dataflow slices, cross-step timelines |
| Solana | Project root with `Anchor.toml`, `idls/<program>.json`, `target/deploy/<program>.so` | Concrete (in-process execution via LiteSVM) | Per-call CU and logs, account diffs, decoded timelines, scenario-isolated VMs, time-warp on the `Clock` sysvar |

The REPL command surface is the same shell. Backend-specific commands are documented in [Solana: Solana runtime](../solana/repl/runtime.md) and [Solana: Scenarios](../solana/repl/scenarios.md).

## Sessions and scenarios

A **session** is the active scenario inside the active project. Adding a step means calling an entry point and recording its effects. A **scenario** is a named branch of the session timeline; scenarios can be created from scratch (`scenario new`) or forked from an existing one at a step boundary (`scenario fork`). On Solana, each scenario owns its own VM and user keypairs, so forks produce independent state.

## What the tool does not do

ilold has no built-in vulnerability detectors. There is no checklist that fires "this is a reentrancy" or "this is a missing access control" automatically. The auditor uses `who`, `info`, `trace`, `slice`, `timeline`, `state`, `step` to investigate, and records findings via `finding` and `note`. See [Roadmap](../roadmap/solana.md) for the Phase 2 detector engine and AST extractor.
