# Solana Known Limitations

These boundaries reflect the current Solana backend. The corresponding [Roadmap entry](../roadmap/solana.md) tracks the Phase 2 work that will lift each of them.

## No static control-flow analysis

Solana programs are loaded as compiled binaries. There is no parsed handler AST and no per-instruction CFG, so the commands that depend on it are not implemented:

- `slice <fn> <var>`: needs Anchor handler AST (see Solana roadmap).
- `trace <fn>`: same reason.
- `sequence`: aliased to `session`; no narrative engine with cross-step dependencies yet.

## `who` is heuristic

`who <field_name>` infers the owning account type via a snake_case → PascalCase fallback against the IDL. Programs with non-conventional naming will miss matches. `who <AccountType>` and `who <ix_name>` use exact lookups and are reliable.

## `time-warp` is one-way for `slot`

`time-warp <delta>` advances `unix_timestamp` linearly for both positive and negative deltas. The `slot` counter only moves forward; negative deltas do not rewind it. Programs that key off `slot` rather than `unix_timestamp` may see inconsistent values after a backward warp.

## `time-warp` is not rewound by `back`

`back` drops the last step and rewinds the VM to the pre-call snapshot of that step, but `time-warp` is a separate side effect on the `Clock` sysvar and is **not** undone. The auditor must reset the clock manually with an inverse `tw` if a later step expects a specific timestamp.

## `save` / `load` regenerates keypairs by default

Without `--with-keypairs`, `save` does not embed the test keypairs. On `load`, a fresh keypair is generated for each user; any PDA derived from a signer pubkey resolves to a different address than the original session.

Pass `save <name> --with-keypairs` to opt into deterministic reload. The resulting JSON contains plaintext keypairs and must not be committed to public repositories. The CLI prints a reminder at both save and load time.

## Legacy `load` without `call_payload`

LoadSession is best-effort for legacy saves missing the `call_payload` field. The timeline restores, but the VM stays at genesis (no replay).

## CPI visibility in the UI

Cross-program CPI calls are exercised correctly by the VM and surface in logs, but the web canvas does not yet have a dedicated visualisation for CPI edges. See [Solana roadmap](../roadmap/solana.md) for the dedicated CPI view.

## Flat bipartite CFG visual

The web canvas renders Solana state as a flat bipartite graph (instructions ↔ accounts). A per-instruction control-flow view is not implemented; see [Solana roadmap](../roadmap/solana.md) for the planned CFG layer.

## Related pages

- [Roadmap: Solana Phase 2](../roadmap/solana.md)
- [Reference: HTTP API](../reference/api-endpoints.md)
