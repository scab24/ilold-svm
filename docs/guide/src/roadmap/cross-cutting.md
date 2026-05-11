# Cross-cutting Roadmap

## MCP server for LLM agent integration

Expose the REPL surface as an MCP (Model Context Protocol) server so an LLM agent can drive an audit programmatically. Each REPL command maps to a typed MCP tool reusing the existing HTTP API. With the server in place, an agent maps a project, proposes call sequences, inspects state, and records findings without a human translating between the model and the REPL.

## Elozer integration

Elozer is our in-house static analyzer. It produces a typed AST for smart-contract source today; a CFG layer needs to be built on top before slicing, taint analysis, and detectors can run on the Solana side. Wiring Elozer into ilold provides that AST foundation and unblocks the items listed in the [Solana roadmap](./solana.md).

## Related

- [Solidity: future work](./solidity.md)
- [Solana: future work](./solana.md)
