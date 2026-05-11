# LLM-driven audit demo

Two walkthroughs for driving ilold with an LLM:

- **Solana via MCP** — natural language prompts handed to Claude Code / Desktop / Cursor / Continue. The LLM picks the tools.
- **Solidity via REPL** — manual command sequence in `ilold explore`. Solidity does not have an MCP server in v1.

Setup is documented in [Reference: MCP server](../reference/mcp.md) and [Getting Started](../getting-started.md). The recipes below assume the backend and the canvas are already running.

## Solana via MCP

Prerequisites:

- `ilold serve tests/fixtures/solana/staking --port 8080` running in one terminal.
- (Optional) `npm run dev` running so the canvas at `http://localhost:5173` reflects each tool call.
- `claude mcp list` shows `ilold ✓ Connected`.

Paste any of the prompts below into the LLM client. The LLM picks the right MCP tools by itself.

### One-liner audit

```
Audit the active Solana program in ilold and hand me the deliverable when done.
```

The LLM will typically explore the program, set up actors, run a happy path, probe admin paths, and export a Markdown audit. Total time: ~30 seconds and ~15 tool calls.

### Senior auditor framing

```
You have ilold available, a workbench that executes smart contracts in a real VM.
Act as a senior security auditor for Solana. A program just landed on your desk
for review. Find anything that could break it in production.

Work as you would in a real audit: map the surface, set up realistic scenarios,
probe whatever attack vectors come to mind, and hand me the deliverable at the end.

Narrate briefly as you go.
```

Best version for video recordings: the LLM stays explanatory, picks tools on its own, and the narration matches what the audience sees in the chat.

### Adversarial framing

```
You are a hacker looking at a freshly deployed Solana program with 50M TVL.
Your only tool is ilold. Find out if you can drain funds without permission,
steal rewards from other users, or break the accounting. Test everything against
the real VM. Walk me through what you find and how you would exploit it.
```

More dramatic tone. Generates findings with attacker phrasing, useful for talks but not for production deliverables.

### Switching programs mid-session

After any of the prompts above, the LLM stays in the same MCP session. To audit a different program in the same backend:

```
Now list all programs available with ilold_programs, switch to a different one,
and repeat the same audit pattern.
```

If you swap the backend (stop `ilold serve` and start it again pointing at a new
project), the MCP keeps working: the next `ilold_programs` call sees the new
workspace.

### Closing the session

```
Summarise everything you did. How many tool calls? Which paths did you cover?
Which paths are still untested? Save the scenario so I can replay it later.
```

The LLM will print the cumulative coverage and call `ilold_save` so the session lives in `~/.ilold/sessions/`.

## Solidity via REPL

Solidity is exposed through the interactive REPL (`ilold explore`), not through MCP. The REPL takes typed commands directly.

Prerequisites:

- `ilold explore tests/fixtures/staking.sol --port 8080` running.
- (Optional) `npm run dev` for the canvas.

### Discovery and analysis

```
f                       List functions
i deposit               Detail one function
v                       List state variables
who totalStaked         Writers and readers of a state variable
seq                     Sequence narrative
```

### Building a session

```
c deposit               Add deposit to the session
c withdraw              Add withdraw
s                       View accumulated state
tr deposit              Trace deposit execution
sl deposit totalStaked  Dataflow slice for totalStaked through deposit
tl totalStaked          Cross-step timeline of totalStaked
```

### Scenarios

```
scenario new attacker
scenario fork attacker 1   Fork active scenario at step 1
scenario list
scenario switch main
```

### Findings and export

```
finding High "reentrancy in withdraw"
note "external call before state update"
status withdraw finding
export                       Generate Markdown deliverable
```

### Inline help

```
?                            Full command reference
sl?                          Quick help for a single command
```

## Recording tips

The Solana MCP path is the one to record on video: the LLM picks tools live and the canvas paints alongside. Pair the chat (50% of the screen) with the canvas (the other 50%) and let the prompt run end to end without pauses.

The Solidity REPL is better for narrated walkthroughs where you want to demonstrate each primitive deliberately.
