# Help and Control

These commands print the command menu, the structured help block for an individual command, or exit the REPL.

## help

`?`, `h`, or `help`

Prints the top-level command menu grouped by category (Session, Programs, Solana runtime, Analysis, Findings, Workspace). Hint at the bottom reminds the auditor that appending `?` to any command prints the full reference block for that command.

```
ilold[staking]> ?

  ilold explore — append ? to any command for inline help (e.g. sl?)

  Session
    c  | call <ix> arg=val acc=user   Concise: keys auto-distributed; signers auto from IDL
    b  | back                          Remove last step from active scenario
    cl | clear                         Reset active scenario steps
       | state                         Decoded view of accounts mutated this session
    s  | session                       Active scenario summary (steps + findings)
  ...
```

## inline help

`<command>?`

Renders the structured help block for that command: purpose, syntax, flags, examples, return shape, and related commands. The block lives in `crates/ilold-cli/src/help.rs::SOLANA_HELP_BLOCKS` and is the canonical reference for every Solana command.

```
ilold[staking]> call?

  c | call

  Purpose
    Run an Anchor instruction against the LiteSVM and append the result as
    a step on the active scenario.

  Syntax
    c <ix> arg=val acc=user   Concise key=value form (signers auto-resolved from IDL)
    c <ix> {json}             Full JSON form: {"args":{...},"accounts":{...},"signers":[...]}

  Flags
    --signer=a,b      Add signers (override IDL defaults)
    --no-signer=name  Remove a default signer (test negative cases)

  Examples
    c stake amount=1000 pool=pool user_stake=alice_stake user=alice
    c initialize_pool reward_rate=10 pool=pool admin=admin
    c stake {"args":{"amount":1000},"accounts":{"pool":"pool", ...}}

  Returns
    StepAdded { step_index, instruction, logs_excerpt, account_diffs_count, compute_units }
    on success, or CallFailed { ... } when the VM rejects.

  See also
    info, pda, state, step, back
```

Lookup is case-insensitive: `CALL?`, `call?`, and `c?` all return the same block.

## sequence

`seq` or `sequence`

Solana has no dedicated cross-step narrative engine yet (Phase 2, see [Roadmap](../../roadmap/solana.md)). `seq` is aliased to `session`: it prints the active scenario summary so an auditor who reaches for `seq` out of habit still gets a useful view.

## quit

`q`, `quit`, or `exit`

Exits the REPL. `Ctrl+D` and `Ctrl+C` also work.

## Notes

- The full list of registered command aliases is enforced by the test `every_solana_command_has_a_help_block` in `crates/ilold-cli/src/help.rs`. New commands without a corresponding HelpBlock break the build.
- The Solidity REPL uses a flat one-line inline-help table (see `print_inline_help`); Solana uses the structured HelpBlock format above. Both respond to the `<cmd>?` trailing syntax.
