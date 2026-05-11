# Cross-cutting Roadmap

## Elozer integration

Elozer is our in-house static analyzer. It produces a typed AST for smart-contract source today; a CFG layer needs to be built on top before slicing, taint analysis, and detectors can run on the Solana side. Wiring Elozer into ilold provides that AST foundation and unblocks the items listed in the [Solana roadmap](./solana.md).

## Related

- [Solidity: future work](./solidity.md)
- [Solana: future work](./solana.md)
