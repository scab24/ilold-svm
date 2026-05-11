# Introduction

ilold is an execution path analyzer and interactive security workbench for smart contracts. It maps every possible path through a protocol (every branch, function combination, and state mutation) and lets users navigate them visually, branch by branch, with an LLM reasoning over each path.

The tool loads a project, builds an in-memory model, and drops the auditor into a REPL backed by a live canvas. Each command answers a question about the protocol: list entry points, add a call to a session, inspect state changes, trace execution flow, slice data dependencies, record findings, export a report. The canvas reflects the same state in a visual graph that the user navigates by clicking, expanding, and forking.

ilold supports two backends:

- **Solidity**: static analysis on top of `solar-compiler`. Contracts are parsed into a typed model; per-function CFGs and path trees drive `info`, `trace`, `slice`, `timeline`, and sequence narratives.
- **Solana**: concrete execution on top of LiteSVM. Programs run inside an in-process VM, accounts are decoded from the program IDL, and timelines are reconstructed from per-step account diffs.

The REPL surface is the same shell for both backends, with backend-specific commands documented in their respective sections.

## Core concept

An audit is a conversation with the code. ilold models that conversation as a **session**: the auditor adds entry-point calls one at a time, and the tool tracks how state accumulates across the sequence. Every analysis command operates either on a single entry point or on the accumulated session state.

Sessions can be branched into named **scenarios**. A scenario is an independent timeline with its own state; on Solana each scenario also owns its own VM and user keypairs.

## Key differentiator

ilold does not detect vulnerabilities automatically. It gives the auditor primitives to drive the analysis: build a sequence, inspect state changes, trace execution flow, slice dependencies, and record findings. The auditor leads, the tool answers questions grounded in the actual structure of the code (Solidity) or the actual runtime behaviour (Solana).

## Where to start

- [Getting Started](./getting-started.md): install and first session.
- [Concepts](./concepts/overview.md): what the tool does and the data pipeline.
- [Solidity Backend](./solidity/overview.md): Solidity REPL, CLI, workflows.
- [Solana Backend](./solana/overview.md): Solana REPL, runtime commands, workflows.
- [Reference](./reference/api-endpoints.md): HTTP API and WebSocket events.
- [MCP server](./reference/mcp.md): drive ilold from an LLM agent.
- [Roadmap](./roadmap/solidity.md): known gaps and future work.
