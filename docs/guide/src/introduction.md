# Introduction

ilold is a Solana program execution path analyzer and interactive security workbench. It maps every reachable instruction path through an Anchor program, executes them on a real LiteSVM, and lets the auditor (or an LLM agent) navigate branches, fork scenarios, and ship a Markdown deliverable.

The tool loads an Anchor workspace, builds an in-memory model from the IDL, boots a LiteSVM with the compiled `.so`, and drops the auditor into a REPL backed by a live canvas. Each command answers a question about the program: list instructions, add a call to a scenario, inspect decoded account state, fork the timeline, record a finding, export the report. The canvas reflects the same state in a visual graph that the user navigates by clicking, expanding, and forking.

Programs run inside an in-process VM, accounts are decoded from the program IDL, and timelines are reconstructed from per-step account diffs. Every `call` runs the compiled program for real — actual compute units, actual logs, actual account state.

## Core concept

An audit is a conversation with the code. ilold models that conversation as a **session**: the auditor adds instruction calls one at a time, and the tool tracks how account state accumulates across the sequence. Every analysis command operates either on a single instruction or on the accumulated session state.

Sessions can be branched into named **scenarios**. A scenario is an independent timeline with its own VM, user keypairs, and account state.

## Key differentiator

ilold does not detect vulnerabilities automatically. It gives the auditor primitives to drive the analysis: build a sequence, inspect state changes, fork scenarios, record findings. The auditor leads, the tool answers questions grounded in actual runtime behaviour against a real VM.

## Where to start

- [Getting Started](./getting-started.md): install and first session.
- [Concepts](./concepts/overview.md): what the tool does and the data pipeline.
- [Solana Backend](./solana/overview.md): REPL, runtime commands, workflows.
- [Reference](./reference/api-endpoints.md): HTTP API and WebSocket events.
- [MCP server](./reference/mcp.md): drive ilold from an LLM agent.
- [Roadmap](./roadmap/solana.md): known gaps and future work.
