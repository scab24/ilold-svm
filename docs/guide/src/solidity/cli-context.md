# CLI: `context`

`ilold context` produces a structured narrative for a single function or for a comma-separated sequence of functions. It runs the same pipeline as `analyze` (parser → CFG → path tree → sequence analysis with cross-contract transitive effects) but emits a focused narrative instead of the project-wide pretty-print.

## Synopsis

```
ilold context <path> [--contract <name>] [--function <name>]
                     [--sequence <f1,f2,...>] [--list]
```

| Flag | Description |
| --- | --- |
| `--contract <name>` | Pick the active contract. Required when the project has more than one. |
| `--function <name>` | Build a function narrative: paths, state reads/writes, internal/external calls, transitive effects, observations. |
| `--sequence <f1,f2,...>` | Build a sequence narrative across the listed functions. |
| `--list` | List functions of the resolved contract with access level and tags, then exit. |

The function/sequence narratives are the same data structures the REPL renders for `i <func>` and `seq` respectively (see `crates/ilold-core/src/narrative/function.rs` and `narrative/sequence.rs`).

## Example: list mode

```
$ ilold context tests/fixtures/staking.sol --list
  Staking — 9 functions

  [P] deposit              external
  [P] withdraw             external
  [P] claimRewards         external
  [R] setRewardRate        external
  [R] pause                external
  [R] unpause              external
  [P] rewardPerToken       public
  [P] earned               public

  Usage:
    ilold context <path> --function <name>
    ilold context <path> --sequence "fn1,fn2"

  Example:
    ilold context <path> --function deposit
    ilold context <path> --sequence "deposit,withdraw"
```

The badges line up with `analyze`: `[P]` public/external entry point, `[R]` restricted/admin-gated, `[S]` system/internal.

## Example: single function

```
$ ilold context tests/fixtures/staking.sol --function withdraw
```

Output is the same `FunctionNarrative` printed by the REPL's `i withdraw`, including transitive effects through the call chain.

## Example: a sequence

```
$ ilold context tests/fixtures/staking.sol --sequence deposit,withdraw,claimRewards
```

The output narrates the per-step writes and the cross-step dependencies (variables shared between consecutive steps).

## Notes

- `context` is read-only; it does not start the API server.
- `--list` short-circuits before computing path trees, so it is cheap on large projects.
- Use [`explore`](./repl/session.md) when you need to iterate; `context` is meant for scripts and one-off questions.
