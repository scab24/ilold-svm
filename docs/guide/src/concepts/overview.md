# What ilold does

ilold is a Solana program audit explorer. It loads an Anchor project, builds an internal model from the IDL, boots a LiteSVM with the compiled program, and exposes that model through an interactive REPL. The auditor adds instruction calls to a **session**, the tool tracks accumulated effects, and analysis commands answer questions about the program without leaving the shell.

## Input and execution model

| Input | Execution model | What you get |
| --- | --- | --- |
| Project root with `Anchor.toml`, `idls/<program>.json`, `target/deploy/<program>.so` | Concrete (in-process execution via LiteSVM) | Per-call CU and logs, account diffs, decoded timelines, scenario-isolated VMs, time-warp on the `Clock` sysvar |

Solana-specific commands are documented in [Solana runtime](../solana/repl/runtime.md) and [Scenarios](../solana/repl/scenarios.md).

## Sessions and scenarios

A **session** is the active scenario inside the active program. Adding a step means calling an instruction and recording its effects. A **scenario** is a named branch of the session timeline; scenarios can be created from scratch (`scenario new`) or forked from an existing one at a step boundary (`scenario fork`). Each scenario owns its own VM and user keypairs, so forks produce independent state.

## What the tool does not do

ilold has no built-in vulnerability detectors. There is no checklist that fires "this is a missing signer" or "this is account confusion" automatically. The auditor uses `who`, `info`, `state`, `timeline`, `step`, `coverage` to investigate, and records findings via `finding` and `note`. See [Roadmap](../roadmap/solana.md) for the Phase 2 detector engine and AST extractor.
