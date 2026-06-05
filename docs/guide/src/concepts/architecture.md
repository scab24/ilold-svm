# Architecture

ilold is a small Rust workspace organised around one pipeline (Solana via LiteSVM), one HTTP/WS layer, one REPL, and one MCP server. The diagram below shows how a typed program model flows from the Anchor IDL down to the canvas and the LLM client.

## Pipeline

```
Anchor.toml + idls/<program>.json + target/deploy/<program>.so
  │
  ▼
ilold-solana-core::ingest          (detect, DetectedProject, AnchorProject)
  │
  ▼
ilold-solana-core::idl + model     (IDL → ProgramDef → ProgramView)
  │
  ▼
ilold-solana-core::execute         (VmHost: LiteSVM boot, snapshots, fork)
  │
  ▼
ilold-solana-core::exploration     (SolanaCommand → SolanaCommandResult)
  │                                 per-step CU, logs, account diffs, decoded timelines
  ▼
ilold-web (HTTP/WS + REST routes)  (/api/cmd, /api/program/*, /api/scenarios, /ws)
  │
  ├─▶ ilold-cli::explore           (REPL, parses input, prints results)
  ├─▶ frontend (Svelte canvas)     (subscribes to /ws, paints state)
  └─▶ ilold-mcp                    (stdio JSON-RPC, exposes 30 tools)
```

There is no static CFG or path tree on Solana today: handlers are bytecode at this point. Anything that requires control-flow analysis (slicing, structural narratives, detector engine) is listed in [Limitations](../solana/limitations.md) and tracked under Phase 2 in the [Roadmap](../roadmap/solana.md).

## Crates

| Crate | Role |
| --- | --- |
| `ilold-cli` | Argument parsing, REPL, output formatting (`src/main.rs`, `explore.rs`, `help.rs`) |
| `ilold-web` | HTTP + WebSocket API consumed by the REPL (via `--attach`) and the web canvas |
| `ilold-session-core` | Backend-agnostic session abstractions (steps, scenarios, canvas patches, access levels, journal) |
| `ilold-solana-core` | Anchor IDL ingest, LiteSVM runtime, instruction execution, timeline reconstruction |
| `ilold-mcp` | Model Context Protocol server exposing the REPL commands as 30 typed tools |
| `ilold-render` | Pretty printers shared by CLI and MCP (byte-identical to legacy CLI prints) |
| `ilold-help` | `SOLANA_HELP_BLOCKS` registry shared by REPL inline help and MCP tool descriptions |

The web canvas (`crates/ilold-web/frontend`) subscribes to the `/ws` stream and stays in sync with whatever the REPL or MCP client does.
