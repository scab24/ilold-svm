# Known Limitations

Limitations are documented per backend, since the boundaries are very different:

- [Solidity: Limitations](../solidity/limitations.md): intraprocedural slicing, assignment-only DEF extraction, modifier placeholder split, tuple destructuring, etc.
- [Solana: Limitations](../solana/limitations.md): no static CFG (no `slice` / `trace` yet), heuristic `who`, `time-warp` semantics, keypair persistence, CPI visibility.

For the planned remediations, see the [Roadmap](../roadmap/solidity.md).
