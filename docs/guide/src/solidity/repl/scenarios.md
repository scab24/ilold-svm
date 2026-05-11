# Scenario Commands

A scenario is a named branch of the session timeline. Every session starts on the default scenario `main`; the auditor can create more scenarios, switch between them, fork an existing one at a specific step, or delete a scenario that is no longer needed. The prompt shows the active scenario as `ilold[Contract/scenario]` when it is not `main`.

## scenario new

`scenario new <name>` (alias: `sc new`)

Creates an empty scenario with no steps. The new scenario is not activated automatically. Use `scenario switch` to make it the active one.

```
ilold[Staking]> sc new reentrancy
  ✓ Created scenario 'reentrancy'
```

**Returns:** `ScenarioCreated { name }`.

## scenario list

`scenario list` (aliases: `scenario ls`, `sc list`)

Lists every scenario in the active session, marking the active one.

```
ilold[Staking]> sc list
  scenarios — 2 total, active: main
        name         steps
    →   main         2
        reentrancy   0
```

**Returns:** `ScenarioList { items: [ScenarioInfo { name, step_count, active }] }`. The CLI renders it inside a framed header box.

## scenario switch

`scenario switch <name>` (alias: `sc switch <name>`)

Activates an existing scenario. All subsequent session and analysis commands operate against its step list. The prompt updates to reflect the new active scenario.

```
ilold[Staking]> sc switch reentrancy
  ✓ Switched: 'main' → 'reentrancy'
ilold[Staking/reentrancy]>
```

**Returns:** `ScenarioSwitched { from, to }`. Switching to the active scenario is idempotent and prints `· Already on scenario '<name>'`.

## scenario fork

`scenario fork <name> [at <N>]` (alias: `sc fork`)

Creates a new scenario branching from the active one. With `at <N>`, the new scenario inherits steps `0..N` from the source scenario; without it, the new scenario inherits the full step list of the source. After forking, the new scenario is activated.

```
ilold[Staking → deposit → withdraw → claimRewards]> sc fork attack-v2 at 1
  ✓ Forked 'main' → 'attack-v2' at step 1
ilold[Staking/attack-v2 → deposit]>
```

**Returns:** `ScenarioForked { from, to, at_step }`.

Forks are useful when the auditor wants to keep an existing line of reasoning intact while testing a divergent path.

## scenario delete

`scenario delete <name>` (aliases: `scenario rm <name>`, `sc delete`, `sc rm`)

Removes a scenario. The active scenario cannot be deleted; switch first.

```
ilold[Staking]> sc delete reentrancy
  ✓ Deleted scenario 'reentrancy'
```

**Returns:** `ScenarioDeleted { name }`.

## Notes

- Solidity scenarios share the same parsed model, so analysis commands stay cheap across forks.
- For the Solana counterpart (each scenario carries its own VM, signers and PDAs), see [Solana: Scenarios](../../solana/repl/scenarios.md).
- The full scenario tree is included in `save` / `load` and in the `export` report.
