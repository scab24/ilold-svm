# CLI: `analyze`

`ilold analyze` parses a Solidity project, runs the full static-analysis pipeline (parser → CFG → path tree → sequence analysis with cross-contract transitive effects), and prints a structured summary to stdout. It is the one-shot view of what `explore` keeps in memory.

## Synopsis

```
ilold analyze <path> [--contract <name>] [--max-seq-depth <N>] [--verbose]
```

| Flag | Default | Description |
| --- | --- | --- |
| `--contract <name>` | (all contracts) | Restrict output to a single contract |
| `--max-seq-depth <N>` | `3` | Depth bound for the sequence tree (call combinations up to N steps) |
| `--verbose` | off | Per-block CFG layout, call-graph edges, full function-behavior breakdown |

`<path>` may be a single `.sol` file or a directory; the walker skips `out`, `cache`, `node_modules`, `lib`, `target`, and dot-prefixed directories.

## Example: default output

```
$ ilold analyze tests/fixtures/staking.sol
Parsed 1 file(s), 2 contract(s)

interface IERC20 (3 functions, 0 state vars)
  [P] external transfer — 1 blocks, 0 edges, 0 paths (0 happy, 0 revert)
  [P] external transferFrom — 1 blocks, 0 edges, 0 paths (0 happy, 0 revert)
  [P] external balanceOf — 1 blocks, 0 edges, 0 paths (0 happy, 0 revert)

contract Staking (9 functions, 11 state vars)
  [S] internal constructor — 2 blocks, 1 edges, 1 paths (1 happy, 0 revert)
  [P] external deposit — 8 blocks, 8 edges, 5 paths (2 happy, 3 revert)
  [P] external withdraw — 8 blocks, 8 edges, 6 paths (2 happy, 4 revert)
  [P] external claimRewards — 6 blocks, 7 edges, 4 paths (4 happy, 0 revert)
  [R] external setRewardRate — 4 blocks, 3 edges, 2 paths (1 happy, 1 revert)
  [R] external pause — 4 blocks, 3 edges, 2 paths (1 happy, 1 revert)
  [R] external unpause — 4 blocks, 3 edges, 2 paths (1 happy, 1 revert)
  [P] public rewardPerToken — 5 blocks, 4 edges, 2 paths (2 happy, 0 revert)
  [P] public earned — 2 blocks, 1 edges, 1 paths (1 happy, 0 revert)
  Sequences (depth 3): 584 total (8 functions: 6 state-changing, 2 read-only)
```

Each function line prints an access badge (`[P]` public/external, `[R]` restricted/admin-gated, `[S]` system/internal), visibility, block/edge counts from the CFG, and the path-tree breakdown (`total`, `happy`, `revert`).

## Example: `--max-seq-depth`

The sequence tree enumerates ordered combinations of entry-point calls up to depth N. Raising the bound surfaces longer interaction patterns that the static analyzer reasons about:

```
$ ilold analyze tests/fixtures/staking.sol --max-seq-depth 5
...
Sequences (depth 5): 37448 total (8 functions: 6 state-changing, 2 read-only)
```

The sequence tree is consumed by the cross-step transitive-effect pass and feeds `seq` in the REPL.

## Example: `--verbose`

`--verbose` adds:

- One line per CFG block (`[id] BlockKind (N stmts)`).
- One line per CFG edge (`src → dst EdgeKind`).
- The intra-contract call graph (`fn → contract.fn` with `internal | external | inherited`).
- A per-function behavior tree with `requires`, `writes`, `calls`, `emits`, and transitions to other functions, including the shared state variables that link them.

The output is meant to be read top-down by a human; `context` (next page) is the machine-readable counterpart.

## Notes

- `analyze` does not require a configured project; it works on raw `.sol` files.
- Interfaces are listed in the contract header but have no sequence tree.
- Errors building the CFG for a single function are reported inline and do not abort the run.

See [Solidity: Limitations](./limitations.md) for the analysis boundaries (intraprocedural slicing, modifier placeholder split, etc.).
