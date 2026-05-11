# Solidity Roadmap

Solidity covers the MVP scope. The items below are tracked enhancements; the section after them is open to ideas.

## Slicer precision

The slicer is intraprocedural and assignment-only. Mutations via `x++`, `delete x`, `arr.push(v)` show up as USEs but not DEFs; tuple destructuring may not surface; forward slices include lexical ancestors and can over-taint.

## Cross-function dataflow

The slicer stops at call boundaries. Following a value through a helper requires `tr <func>` (manual inlining) or a separate run on the helper.

## Modifier placeholder split

Modifier bodies are split at the first top-level `_;`. Nested placeholders fall back to "before" code.

## Sequence depth bound

`--max-seq-depth` defaults to 3. Deeper bounds grow combinatorially; no change planned.

## Open to ideas

The roadmap is not closed. Examples of integrations we have considered but not started:

- **Foundry**: today ilold reads Foundry projects (the `multi/` and `recursive/` fixtures are Foundry layouts) but does not invoke `forge build` or `forge test`. Possible directions include using `forge build` artefacts as an alternative ingest path, replaying PoCs from findings via `forge test --debug`, or cross-linking traces.
- **Cross-tool reports**: emitting findings in a format consumable by other audit pipelines.
- **New analysis passes**: anything that fits the CFG + path-tree model.

If you have a use case the current backend does not cover, open an issue or reach out.
