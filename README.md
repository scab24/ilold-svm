# ilold

Solana program execution path analyzer and interactive security workbench. It maps every reachable instruction path through an Anchor program, executes them on a real LiteSVM, and lets the auditor (or an LLM agent) navigate branches, fork scenarios, and ship a Markdown deliverable.

![ilold](images/Ilold.jpg)

## What it does

ilold loads an Anchor workspace, builds an in-memory model from the IDL, boots a LiteSVM with the compiled `.so`, and drops the auditor into an interactive REPL backed by a live web canvas. Each command answers a question about the program: list instructions, add a call to a scenario, inspect decoded account state, fork the timeline, record a finding, export the report. The canvas reflects the same state in a visual graph that the user navigates by clicking, expanding and forking.

ilold also ships a Model Context Protocol (MCP) server so any LLM client (Claude Code, Cursor, Continue) can drive the audit end to end while the canvas reflects every step in real time.

## Quick start

Build from source:

```
git clone https://github.com/scab24/ilold.git
cd ilold
cargo build --release
```

The binary is at `target/release/ilold` with three subcommands: `serve`, `explore`, and `mcp`.

## Solana backend

Anchor programs execute against a LiteSVM-backed engine. Every `call` runs the compiled program in the VM, so the auditor sees real compute units, real logs, and real account state. Scenarios can be forked from any step and replayed deterministically with `save --with-keypairs` / `load`.

![Solana canvas](images/diagram_solana.png)

```
./target/release/ilold explore tests/fixtures/solana/staking --port 8080
```

Then in the REPL:

```
ilold[staking]> f
ilold[staking]> info stake
ilold[staking]> users new alice
ilold[staking]> c stake amount=1000 pool=pool user_stake=alice_stake user=alice
ilold[staking]> state
ilold[staking]> coverage
ilold[staking]> finding High "..."
ilold[staking]> export
```

### LLM-driven audit (MCP)

Register the MCP server once in your client:

```
claude mcp add ilold --transport stdio -- \
  ./target/release/ilold mcp --server-url http://127.0.0.1:8080
```

Then ask the LLM in natural language:

> Audit the active Solana program. Map the surface, set up a realistic scenario, probe whatever attack vectors come to mind, and hand me the deliverable at the end.

The LLM picks the right tools, the backend executes them in LiteSVM, and the canvas reflects each step in real time. The MCP server is agnostic to the active program: a single registration works against multi-program workspaces, the LLM switches with `ilold_use <program>`.

## Documentation

The published book lives at **[scab24.github.io/ilold](https://scab24.github.io/ilold/)**. Sources are in [`docs/guide/`](docs/guide/src/SUMMARY.md); build it locally with:

```
mdbook serve docs/guide --open
```

Key pages:

- [Introduction](docs/guide/src/introduction.md)
- [Getting Started](docs/guide/src/getting-started.md)
- [Solana Backend](docs/guide/src/solana/overview.md)
- [MCP server](docs/guide/src/reference/mcp.md)
- [Roadmap](docs/guide/src/roadmap/solana.md)

## Status

MVP scope shipped:

- Typed graph from Anchor IDL with discriminator + admin-gating + coupling metadata
- Executable scenarios with fork from any step, save/load, runtime overlay
- LLM-aware REPL with structured `?` help per command
- MCP server with 30 typed tools agnostic to the active program
- Source viewer with "open in IDE" deep links
- Markdown audit deliverable with severity matrix and methodology

Documented future work (see [Roadmap](docs/guide/src/roadmap/solana.md)):

- AST + CFG layer for Anchor handlers via Elozer, our in-house static analyzer
- Detector engine measured against the public sealevel-attacks corpus
- Pre-built attack-pattern query catalog

For the legacy Solidity backend, see the standalone [`ilold-evm`](https://github.com/scab24/ilold-evm) repo.

## Project layout

Single Rust monorepo:

```
crates/
  ilold-cli           Interactive shell + serve/explore/mcp CLI
  ilold-solana-core   Solana engine (Anchor IDL + LiteSVM + runtime overlay)
  ilold-session-core  Shared session, scenarios, findings, export
  ilold-web           REST + WebSocket server (Svelte frontend inside)
  ilold-mcp           MCP server for LLM agent integration
  ilold-render        Pretty printers shared by CLI and MCP
  ilold-help          Tool registry shared by REPL help and MCP
tests/
  fixtures/solana/    Anchor fixtures (binaries committed)
  scenarios/          Bash + Python end-to-end suite
docs/guide/           Public mdbook documentation
```

## License

ilold is licensed under the [GNU Affero General Public License v3](LICENSE). The AGPL section 13 (remote network interaction) applies: if you run a modified version of ilold on a network server, you must offer the modified source to users of that server.

Copyright (C) 2026 scab24.
