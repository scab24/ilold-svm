# Testing Ilold against a Solana program

This page is the practical guide for running, exercising and validating
Ilold's Solana support. It complements `solana-support.md` (which
explains the conceptual model) and is meant to be read by someone who
wants to break the tool, contribute fixes, or audit a real program.

## 1. Build + serve

```bash
cargo build --release --bin ilold -p ilold-cli
./target/release/ilold serve --port 8080 tests/fixtures/solana/staking
```

`tests/fixtures/solana/staking` is the canonical Anchor fixture: a
toy staking program with `initialize_pool`, `stake`, `unstake`,
`add_rewards`, `claim_rewards`. The IDL lives in `idls/staking.json`
and the compiled `.so` is committed to `bin/staking.so` so the suite
runs even on machines without the Anchor toolchain.

## 2. The REPL

In a second terminal:

```bash
./target/release/ilold explore --base-url http://127.0.0.1:8080 --contract staking
```

Type `?` for the help. Two-terminal flows (one for `serve`, one for
`explore`) are the supported mode — both stay in sync via REST + WS.

## 3. Command surface

### Session

| Command | Description |
| --- | --- |
| `call <ix> arg=val acc=user` | Concise key-value call. Unmapped names become local keypairs. |
| `call <ix> {json}` | Full payload `{args, accounts, signers}` form. |
| `back` / `b` | Pop the last step and rewind the VM to its pre-Call snapshot. |
| `clear` / `cl` | Drop all steps in the active scenario; rewinds the VM to genesis. |
| `state` / `s` | Decoded view of accounts mutated this session. |
| `session` / `s` | Active scenario summary (steps + findings count). |
| `step <i>` / `st <i>` | Re-inspect step `i`: CU, logs, decoded diffs. |

### Scenarios

| Command | Description |
| --- | --- |
| `scenario new <name>` / `sc new` | Create an empty scenario. |
| `scenario fork <name> [step]` | Fork at step N (or HEAD). The branch VM is rewound to that step's pre-Call snapshot. |
| `scenario switch <name>` | Activate a scenario. Each scenario has its own VM and users. |
| `scenario list` | Show all scenarios with active marker + step count. |
| `scenario delete <name>` | Remove a scenario (cannot delete the active one). |

### Solana runtime (no Solidity counterpart)

| Command | Description |
| --- | --- |
| `users new <name> [lamports]` | Create a keypair and airdrop SOL into it. Default: 10 SOL. |
| `airdrop <name> <lamports>` | Top up an existing keypair. |
| `time-warp <delta_seconds>` / `tw` | Advance the `Clock` sysvar. |
| `pda <ix>` | List PDAs declared by an instruction in the IDL. |
| `inspect <pubkey>` | Decode an account by Anchor discriminator. |

### Analysis

| Command | Description |
| --- | --- |
| `info <ix>` / `i <ix>` | Args, accounts, flags, discriminator. |
| `funcs` / `f` / `funcs-all` / `fa` | Instruction list (compact / verbose). |
| `vars` / `v` / `vars-all` / `va` | Account types declared in IDL. |
| `who <account_type>` | Instructions referencing this type (heuristic snake_case → PascalCase). |
| `timeline <pubkey>` / `tl` | Cross-step mutation history with before/after decoded. |

### Findings & workspace

| Command | Description |
| --- | --- |
| `finding <severity> <title>` / `fi` | Record a finding. Severities: critical, high, medium, low, info. |
| `note <text>` / `n` | Annotation on the active sequence. |
| `status <ix> <state>` | Mark instruction reviewed / suspicious / etc. |
| `findings` / `fl` | List recorded findings. |
| `export` / `ex` | Markdown report aggregating findings + steps from ALL scenarios. |
| `save <name>` / `load <name>` | Persist / restore a session JSON in `~/.ilold/sessions/`. |

## 4. Differences vs Solidity

The REPL is the same shell but the backends are very different. This
table says exactly what overlaps, what diverges and what is not yet
implemented.

| Concept | Solidity | Solana |
| --- | --- | --- |
| Entry point | function on a contract | instruction on a program |
| Persistent state | contract state variables | accounts owned by the program |
| Caller identity | `msg.sender` (implicit) | signers passed by the client |
| `who <X>` | finds writers / readers of a state variable (uses CFG) | finds instructions referencing an account type (heuristic on the IDL) |
| `timeline <X>` | mutation history of a state variable | mutation history of an account pubkey, decoded |
| `step <i>` | reads the step's narrative from the saved CFG path | re-prints the runtime trace (CU, logs, account diffs) of step i |
| `slice <fn> <var>` | backward / forward dataflow on the function CFG | **not implemented** — needs a handler AST extractor (Phase 2) |
| `trace <fn>` | full execution flow with modifier inlining | **not implemented** — same reason |
| `sequence` | narrative of cross-step dependencies | aliased to `session` (no narrative engine yet) |
| Execution model | symbolic (CFG + paths) | concrete (LiteSVM with the real BPF binary) |
| Save/Load | restores step list + paths; no VM | restores step list + replays Calls against a fresh VM (T-R39) |
| `Back` | drops the last step from the timeline only | drops the step AND restores the VM to the pre-Call snapshot (T-R33) |

## 5. Smoke test suite

```bash
bash tests/scenarios/run.sh
```

12 scenarios under `tests/scenarios/`. Each spawns a fresh `ilold serve`
on port 8081 (override with `ILOLD_TEST_PORT`) and aggregates pass/fail.
Adding a scenario:

1. Create `tests/scenarios/NN-name.sh` (use `01-happy-path.sh` as
   template, source `_lib.sh`).
2. Make it executable: `chmod +x …`.
3. Re-run the runner; it picks up `[0-9][0-9]-*.sh` automatically.
4. Optionally provide a python websockets script next to the bash
   files; if the runner finds a `.py` it will run it after the bash
   suite when `python3 + websockets` is installed.

The current set covers:

- Happy path with state accumulation.
- Four negative attack vectors that must be rejected by the program
  (re-init, claim without stake, unstake overflow, non-admin
  add_rewards).
- Fork isolation: a branch VM rewinds to the fork point and changes
  there do not leak to main.
- `Back` rewinds the VM (T-R33).
- 50 consecutive Calls execute (T-R40 blockhash rotation).
- `Save → Clear → Load` reconstructs the VM (T-R39).
- Findings recorded in any scenario surface in the markdown export.
- WebSocket broadcast count + payload (T-R37 runtime metadata).

## 6. Reproducing an audit walkthrough manually

```text
ilold[staking]> users new admin 100000000
ilold[staking]> users new pool 2000000
ilold[staking]> users new alice 50000000
ilold[staking]> users new alice_stake 2000000
ilold[staking]> call initialize_pool reward_rate=10 pool=pool admin=admin
ilold[staking → initialize_pool]> call stake amount=1000 pool=pool user_stake=alice_stake user=alice
ilold[staking → … → stake]> step 1
ilold[staking → … → stake]> who Pool
ilold[staking → … → stake]> timeline <pool-pubkey>
ilold[staking → … → stake]> finding High "missing reentrancy guard"
ilold[staking → … → stake]> findings
ilold[staking → … → stake]> export
ilold[staking → … → stake]> save my-audit
ilold[staking → … → stake]> clear
ilold[staking]> load my-audit
```

## 7. Frontend testing

```bash
cd crates/ilold-web/frontend
npm install
npm run dev      # vite proxy points /api and /ws to http://localhost:8080
```

In another terminal `./target/release/ilold serve --port 8080 …` and
open `http://localhost:5173/contract/staking`. The page subscribes to
the same WS stream the CLI consumes, so any command run from the REPL
shows up live (with full CU / diffs / logs after T-R37).

To stress the UI specifically (race fix in audit round 10):

1. Open `/contract/staking` (Solana).
2. Click a different contract from the project map mid-load (before
   the page renders).
3. The new contract must render — no `kind=solidity` regression on a
   Solana program.

## 8. Known limitations as of this writing

- `slice`, `trace`, `sequence` analysis for Solana require a handler
  AST and are deferred (Phase 2 in the SDD roadmap).
- `Save → Load` regenerates user keypairs on the next session, so
  programs that hash signer pubkeys into PDAs will see different
  derived addresses after `load`. Tracked under
  `docs/internal/sdd-roadmap.md` — topic 3.
- `time-warp` advances `unix_timestamp` linearly; negative deltas do
  not reverse `slot`.
- `who` uses snake_case → PascalCase heuristic to map account fields
  to types; non-conventional naming will miss.
- The CFG visual layout for Solana is a flat bipartite (instructions
  vs accounts). A redesign is queued pending design feedback.
