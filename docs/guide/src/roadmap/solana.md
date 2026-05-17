# Solana Roadmap

Items below are tracked work without committed dates.

## AST via Elozer

Plug Elozer, our in-house static analyzer, into ilold to produce a typed AST for the program source: account validation, state writes, constraints, CPI sites. Foundation for everything below.

## CFG on top of the AST

Build the control-flow graph layer on the Elozer AST. Unlocks `slice`, `trace`, and structural narratives over Anchor handlers.

## Detector engine

Detectors for known Sealevel attack patterns (missing signer checks, missing owner checks, account confusion, arithmetic overflow, reinit, PDA seed collision) measured against the public sealevel-attacks corpus. Depends on AST + CFG.

## LiteSVM register-tracing bridge

Record concrete values at each VM instruction boundary so the dynamic trace can confirm or refute hypotheses produced by the static layer.

## CFG visual on the canvas

The web canvas renders Solana state today as a flat bipartite graph (instructions ↔ accounts). The redesigned view adds a per-instruction control-flow layer with branch nodes and constraint annotations once the CFG layer above is in place.

## CPI graph in the UI

The runtime already records CPI edges (`coverage` surfaces them in text). A dedicated CPI view in the canvas is the next visual step.

## Sequence narrative

`sequence` is aliased to `session` today. A true narrative engine on top of the CFG + `coupling` aggregate is tracked under Phase 2.

## Open to ideas

The Solana side is younger and the roadmap above is the current shape, not a fixed plan. Examples of directions we are open to:

- New analysis passes once Elozer's AST and the CFG layer are in place.
- Integrations with other Solana tooling (anchor-cli, sealevel-attacks corpus consumers, custom IDL extensions).
- Alternative VMs or replay engines beyond LiteSVM if a use case justifies it.

If you have a concrete use case the current backend does not cover, open an issue or reach out.

## Related

- [Solana: Limitations](../solana/limitations.md)
- [Cross-cutting](./cross-cutting.md)
