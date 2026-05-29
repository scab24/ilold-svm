# Introduction

ilold is a static analysis tool for Solidity smart contracts that provides an interactive REPL for exploring execution paths, state mutations, and data flow. Instead of producing a batch report and leaving the auditor to interpret it, ilold builds an in-memory model of the contract and lets you query it interactively, building up call sequences step by step.

## Core Concept

A security audit is a conversation with the code. ilold models that conversation as a **session**: you add function calls one at a time, and the tool tracks how state accumulates across the sequence. At any point you can ask questions -- who writes this variable? what does the data flow look like? what is the full execution tree? -- and get answers scoped to the contract's actual structure.

The session is the central abstraction. Every analysis command operates either on a single function or on the accumulated session state, so the results stay grounded in realistic execution scenarios rather than theoretical possibilities.

## Pipeline

```
Foundry project
  |
  v
Parser (solc via foundry-compilers)
  |
  v
Model (contracts, functions, modifiers, state variables, resolved cross-contract calls)
  |
  v
CFG (control flow graph per function)
  |
  v
Analysis
  ├── trace    — execution flow tree with modifier inlining
  ├── slice    — backward/forward dataflow analysis
  ├── timeline — cross-step mutation history
  └── narrative — function and sequence summaries
```

The parser produces a typed model of each contract. From the model, ilold builds a control flow graph per function, then layers analysis passes on top. The REPL exposes these passes as individual commands.

## Dependency Graph

Before drilling into a single function, ilold maps how the whole project fits together. Because the solc frontend resolves cross-contract calls to their real targets, it can build a **contract dependency graph** with three relationship kinds:

- **inherits** — one contract extends another.
- **calls** — a function calls into another contract.
- **holds** — a contract keeps a state variable of another contract's type.

Cycles are condensed and the graph is sorted into **topological layers**, giving a reading order: dependency-free contracts (interfaces, libraries) first, the contracts that build on them next. The same data drives `deps`/`usedby` in the REPL, `analyze --deps` on the CLI, and the interactive graph that opens as the web canvas — color by source subsystem, line color by relationship.

## Key Differentiator

Traditional static analyzers run a fixed set of detectors and produce a flat list of warnings. ilold does not detect vulnerabilities automatically. Instead, it gives the auditor tools to explore the contract interactively: build a call sequence, inspect state changes, trace execution flow, slice data dependencies. The auditor drives the analysis and records findings as they go.

This is closer to how manual audits actually work -- following a thread through the code, checking what happens when functions are called in a specific order, and documenting what you find.
