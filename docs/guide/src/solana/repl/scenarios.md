# Scenario Commands

Scenarios are independent branches of the session. On Solana, **each scenario owns its own VM and its own keypair set**: a fork carries the parent's VM state up to the fork step, then diverges. This is the core mechanism for testing "what happens if step 2 fails differently?" without losing the original timeline.

## scenario new

`sc new <name>` or `scenario new <name>`

Creates an empty scenario with a fresh VM (the program is reloaded, no users yet). The new scenario is not activated automatically.

```
ilold[staking]> sc new attack
  ✓ scenario attack created
```

**Returns:** `ScenarioCreated { name }`.

## scenario list

`sc list` (aliases: `sc ls`, `scenario list`, bare `sc`)

Lists every scenario in the active session with the active marker and step count.

```
ilold[staking]> sc list
  [S] main (3 steps) ← active
  [S] attack (0 steps)
```

**Returns:** `ScenarioList { items: [ScenarioInfo { name, step_count, active }] }`.

## scenario switch

`sc switch <name>`

Activates an existing scenario. The VM, keypairs, and step list are swapped to that scenario's state.

```
ilold[staking]> sc switch attack
  → main → attack
ilold[staking/attack]>
```

**Returns:** `ScenarioSwitched { from, to }`.

## scenario fork

`sc fork <name> [step]`

Creates a new scenario branching from the active one. With `[step]`, the new scenario inherits steps `0..step` and the VM is rewound to the **pre-call snapshot** of that step. Without `[step]`, the fork inherits the full step list and the VM state at HEAD. The new scenario is activated after forking.

```
ilold[staking → … → claim_rewards]> sc fork attack-v2 1
  ✓ forked main → attack-v2 at step 1
ilold[staking/attack-v2 → initialize_pool]>
```

**Returns:** `ScenarioForked { from, to, at_step }`.

The fork keeps the parent's keypair definitions so PDAs derived from them resolve to the same addresses (provided you opt into deterministic keypairs via `save --with-keypairs`; see [Workspace](./workspace.md)).

## scenario delete

`sc delete <name>` (aliases: `sc rm <name>`)

Removes a scenario. The active scenario cannot be deleted; switch first.

```
ilold[staking]> sc delete attack
  ✓ scenario attack deleted
```

**Returns:** `ScenarioDeleted { name }`.

## Notes

- `back` and `clear` rewind the VM of the **active scenario only**, never the fork's parent. Diverging via `fork` is the only way to keep both timelines side by side.
- `time-warp` is a per-scenario side effect on the `Clock` sysvar, but is not reverted by `back`.
- See [Scenarios and forks](../workflows/scenarios.md) for an end-to-end workflow.
