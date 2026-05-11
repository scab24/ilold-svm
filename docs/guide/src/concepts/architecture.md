# Architecture

ilold is split into a handful of crates with clear responsibilities. The diagram below shows the pipeline for each backend; both meet at the shared web layer (`ilold-web`) and the REPL frontend (`ilold-cli`).

## Solidity pipeline

```
.sol files
  │
  ▼
ilold-core::parse::solar_frontend   (SolarParser)
  │
  ▼
ilold-core::model                   (Project, ContractDef, FunctionDef, ...)
  │
  ▼
ilold-core::cfg::builder            (CfgBuilder)
  │
  ▼
ilold-core::pathtree::walker        (build_path_tree)
  │
  ▼
ilold-core::sequence::analysis      (analyze_sequences, analyze_project)
  │
  ▼
ilold-core::narrative + slicing     (info, trace, slice, timeline)
```

`analyze` and `context` run this pipeline once and print to stdout. `serve` and `explore` keep the model in memory and expose it through the HTTP/WS API in `ilold-web`.

## Solana pipeline

```
Anchor.toml + idls/<program>.json + target/deploy/<program>.so
  │
  ▼
ilold-solana-core::ingest           (detect, AnchorProject)
  │
  ▼
ilold-solana-core::runtime          (LiteSVM-backed engine)
  │
  ▼
ilold-solana-core::exploration      (SolanaCommand → SolanaCommandResult)
  │                                  per-step CU, logs, account diffs, decoded timelines
  ▼
ilold-web (shared HTTP/WS layer)    (/api/cmd, /api/program/*, /ws)
  │
  ▼
ilold-cli::explore                  (REPL, parses input, prints results)
```

There is no static CFG or path tree on Solana today: programs are bytecode at this point. Anything that requires control-flow analysis (`slice`, `trace`, `sequence` narrative) is listed in [Solana: Limitations](../solana/limitations.md) and tracked under Phase 2 in the [Roadmap](../roadmap/solana.md).

## Shared layer

| Crate | Role |
| --- | --- |
| `ilold-cli` | Argument parsing, REPL, output formatting, key bindings (`crates/ilold-cli/src/main.rs`, `explore.rs`, `help.rs`) |
| `ilold-web` | HTTP + WebSocket API consumed by both the REPL (via `--attach`) and the web canvas |
| `ilold-session-core` | Shared session abstractions (steps, scenarios, canvas patches) |
| `ilold-core` | Solidity model, CFG, slicer, narrative, sequence analysis |
| `ilold-solana-core` | Anchor IDL ingest, LiteSVM runtime, instruction execution, timeline reconstruction |

The web canvas (`crates/ilold-web/frontend`) subscribes to the `/ws` stream and stays in sync with whatever the REPL does.
