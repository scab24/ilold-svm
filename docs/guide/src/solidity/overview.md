# Solidity Backend Overview

The Solidity backend is built on top of `solar-compiler` (parser) and a set of analysis passes in `ilold-core`. It supports both a one-shot CLI (`analyze`, `context`) and the interactive REPL (`explore`, `serve`).

## What it parses

`crates/ilold-cli/src/main.rs::collect_sol_files` walks the input path:

- A single `.sol` file is loaded as-is.
- A directory is walked recursively; the directories `out`, `cache`, `node_modules`, `lib`, `target`, `.git`, `.svelte-kit` and any dot-prefixed directory are skipped.

Once the project is parsed, ilold builds a per-function CFG via `CfgBuilder::build_with_project` and a path tree via `build_path_tree` (see `crates/ilold-core/src/cfg/builder.rs` and `pathtree/walker.rs`). Inheritance is resolved transitively, so inherited functions and state variables show up in `funcs-all` / `vars-all` and in `info` output.

## What you can do with it

| Surface | Purpose |
| --- | --- |
| [`analyze`](./cli-analyze.md) | One-shot pretty-print of every contract: functions, CFG and path-tree stats, sequences up to `--max-seq-depth`, optional verbose function behavior breakdown |
| [`context`](./cli-context.md) | Generate machine-readable narratives for a function or a comma-separated sequence |
| [`serve`](./repl/workspace.md) | Start the HTTP/WS server only, no REPL: feed the web canvas |
| [`explore`](./repl/session.md) | Interactive REPL, with the HTTP/WS API running alongside |

## REPL command groups

The REPL has six command groups, all documented in their own pages:

- [Session](./repl/session.md): `c/call`, `b/back`, `cl/clear`, `s/state`, `seq/sequence`, `st/step`, `ss/session`.
- [Analysis](./repl/analysis.md): `w/who`, `i/info`, `tr/trace`, `tl/timeline`, `sl/slice`.
- [Contract](./repl/contract.md): `f/functions`, `fa/funcs-all`, `v/vars`, `va/vars-all`, `ct/contracts`, `use`.
- [Findings](./repl/findings.md): `fi/finding`, `n/note`, `status`, `fl/findings`, `ex/export`.
- [Scenarios](./repl/scenarios.md): `sc/scenario` (`new`, `list`, `switch`, `fork`, `delete`).
- [Workspace](./repl/workspace.md): `save`, `load`, `browser`, `q/quit/exit`, `?/help`.

## Workflows

Two end-to-end walkthroughs are included:

- [Audit walkthrough](./workflows/audit-walkthrough.md): full session against a Staking contract.
- [Taint analysis](./workflows/taint-analysis.md): forward slicing of user-controlled parameters.
