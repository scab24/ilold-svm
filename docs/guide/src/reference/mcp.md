# MCP Server

ilold ships an MCP (Model Context Protocol) server that exposes the Solana REPL as a set of typed tools. Any MCP-compatible client (Claude Code, Claude Desktop, Cursor, Continue) can invoke those tools to drive an audit programmatically: list instructions, call them against the live LiteSVM, inspect state, record findings, and export the deliverable. The MCP server is a thin transport on top of the existing HTTP API; it adds no new domain logic.

## Architecture

```
LLM client  ──── stdio ────►  ilold mcp  ──── HTTP ────►  ilold serve  ────►  LiteSVM
                                                              │
                                                              └──── WebSocket ────►  Web canvas (optional)
```

The MCP client launches `ilold mcp` as a local subprocess and talks to it over stdio (newline-delimited JSON-RPC). The MCP process is stateless: each `tools/call` translates the arguments into a `SolanaCommand` and forwards it to a running `ilold serve` instance via `POST /api/cmd`. The same backend broadcasts canvas patches over WebSocket, so a browser tab connected to the web canvas reflects every step the LLM takes.

Only Solana is supported in v1. The MCP server refuses to start when the backend reports `kind != "solana"`.

## Setup

Two processes need to be running:

1. **Backend**: an `ilold serve` instance pointing at the project to audit.

   ```
   ilold serve tests/fixtures/solana/staking --port 8080
   ```

   The MCP server defaults to `http://127.0.0.1:8080`, so any free port works as long as `--server-url` matches.

2. **MCP client**: configure the LLM client to spawn `ilold mcp` (see the client snippets below). The client launches the subprocess on demand and tears it down when the session ends.

The `ilold` binary must be on the client's `PATH`. If it is not, use the absolute path returned by `which ilold` in the `command` field.

## CLI reference

```
ilold mcp [OPTIONS]
```

| Flag | Required | Default | Description |
| --- | --- | --- | --- |
| `--server-url <URL>` | no | `http://127.0.0.1:8080` | Base URL of the `ilold serve` instance. Environment variable: `ILOLD_SERVER_URL`. |
| `--contract <NAME>` | no | unset | Optional initial active program. When unset the LLM (or the user) must call `ilold_use <program>` before any other tool. Pre-setting it is handy when the workspace has a single program. Environment variable: `ILOLD_CONTRACT`. |
| `--narration` | no | off | Emit a `notifications/progress` MCP message before each tool call describing intent (for example `Calling \`stake\` with amount=1000`). Environment variable: `ILOLD_NARRATION`. |

The MCP server is agnostic to the active contract. A single registration in the client works against multi-program workspaces: the LLM lists programs with `ilold_programs` and then fixes the active one with `ilold_use`.

The MCP transport reserves stdout for JSON-RPC; logs and panics go to stderr.

## Client configuration

Every snippet below assumes the backend is running on `http://127.0.0.1:8080`. The MCP server is registered once and stays agnostic to the active program — the LLM calls `ilold_use <program>` to switch contract during the session. Pre-setting `--contract <name>` is optional and only seeds the initial value.

### Claude Code

Two options. The first is project-scoped (`.mcp.json` at the repository root, checked into version control); the second is the `claude mcp add` CLI which writes to `~/.claude.json` by default.

`.mcp.json`:

```json
{
  "mcpServers": {
    "ilold": {
      "command": "ilold",
      "args": [
        "mcp",
        "--server-url", "http://127.0.0.1:8080"
      ]
    }
  }
}
```

Add `"--contract", "<name>"` to the `args` list to pre-set the initial active program.

Equivalent CLI form:

```
claude mcp add --transport stdio ilold -- ilold mcp --server-url http://127.0.0.1:8080
```

### Claude Desktop

Edit `claude_desktop_config.json` (Developer → Edit Config in the desktop settings):

- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "ilold": {
      "command": "ilold",
      "args": [
        "mcp",
        "--server-url", "http://127.0.0.1:8080"
      ]
    }
  }
}
```

Restart Claude Desktop after saving. The MCP indicator in the input box lists `ilold` and its tools when the connection is healthy. Append `"--contract", "<name>"` to `args` to pre-set an initial program.

### Cursor

Place the file at `.cursor/mcp.json` (project) or `~/.cursor/mcp.json` (global):

```json
{
  "mcpServers": {
    "ilold": {
      "command": "ilold",
      "args": [
        "mcp",
        "--server-url", "http://127.0.0.1:8080"
      ]
    }
  }
}
```

Optional `env` and `envFile` keys are supported by Cursor for passing environment variables. Append `"--contract", "<name>"` to `args` to pre-set an initial program.

### Continue

Continue uses YAML. Edit `~/.continue/config.yaml`:

```yaml
mcpServers:
  - name: ilold
    type: stdio
    command: ilold
    args:
      - mcp
      - --server-url
      - http://127.0.0.1:8080
```

Append `- --contract` and `- <name>` to `args` to pre-set an initial program.

## Tools

The registry is derived at startup from `crates/ilold-help/src/lib.rs::SOLANA_HELP_BLOCKS`. The table below lists every exposed tool with a one-line summary. Each tool returns the matching `SolanaCommandResult` variant as structured JSON plus a pretty-printed text block identical to the REPL output.

### Discovery (read-only)

| Tool | Purpose |
| --- | --- |
| `ilold_programs` | List every program detected in the workspace. |
| `ilold_funcs` | List the instructions exposed by the active program. |
| `ilold_funcs_all` | Same list with admin-gating and coupling hints. |
| `ilold_info` | Detail one instruction: args, accounts, signers, PDAs, discriminator. |
| `ilold_vars` | List declared account types with their Anchor discriminators. |
| `ilold_pda` | List the PDAs declared by an instruction (seeds, bumps). |
| `ilold_who` | Resolve a query against the IDL (account type, instruction, or field). |
| `ilold_coupling` | List instruction pairs that share a writable account. |

### Session (mutate the timeline)

| Tool | Purpose |
| --- | --- |
| `ilold_call` | Run an Anchor instruction against LiteSVM and append the result as a step. |
| `ilold_back` | Remove the last step from the active scenario and rewind the VM. |
| `ilold_clear` | Reset the active scenario steps and the underlying VM state. |
| `ilold_state` | Decoded view of every account mutated during the active scenario. |
| `ilold_session` | Active scenario summary: steps, findings, notes. |
| `ilold_step` | Re-inspect one step: CU, logs, decoded diffs. |

### Runtime (mutate the VM)

| Tool | Purpose |
| --- | --- |
| `ilold_users` | List every named keypair in the active scenario. |
| `ilold_users_new` | Create a new keypair and airdrop the initial lamports. |
| `ilold_airdrop` | Top up an existing keypair with extra lamports. |
| `ilold_time_warp` | Advance or rewind the Clock sysvar. |
| `ilold_inspect` | Read a VM account by pubkey and decode it via the Anchor discriminator. |

### Analysis

| Tool | Purpose |
| --- | --- |
| `ilold_timeline` | Cross-step mutation history of an account, decoded. |
| `ilold_coverage` | Aggregated runtime metrics over the active scenario (calls, failures, CU stats, CPI edges). |

### Scenarios

| Tool | Purpose |
| --- | --- |
| `ilold_scenario` | Manage scenarios: create, list, switch, fork, delete. |

### Findings and journal

| Tool | Purpose |
| --- | --- |
| `ilold_finding` | Record a security finding tied to the latest step. |
| `ilold_findings` | List every finding recorded in the active scenario. |
| `ilold_note` | Attach a free-form annotation to the active scenario. |
| `ilold_status` | Set the review status of an instruction: open, reviewed, finding. |
| `ilold_export` | Generate the audit deliverable (Markdown). |

### Workspace

| Tool | Purpose |
| --- | --- |
| `ilold_use` | Set the active program for the rest of the MCP session. Every other tool call routes to this program. |
| `ilold_save` | Serialise the active scenario to `~/.ilold/sessions/<name>.json`. |
| `ilold_load` | Restore a scenario JSON from disk and replay it into the VM. |

Total: 30 tools. The REPL meta commands (`?`, `help`, `quit`, `browser`, `seq`) are intentionally excluded: the MCP client discovers tools via `tools/list`, the subprocess exits on stdin EOF, and the canvas URL is already on the human side.

## Switching programs

Multi-program workspaces are handled at runtime, not at registration time:

1. `ilold_programs` lists every program detected by the backend. The active one is marked.
2. `ilold_use <program>` sets the active program. The handler validates the name against `/api/project/map` and rejects unknown names.
3. Subsequent tool calls (`ilold_funcs`, `ilold_call`, etc.) route to the active program automatically.

If no contract is active (no `--contract` flag and no prior `ilold_use` call), every tool other than `ilold_programs` and `ilold_use` returns a clear error asking the LLM to set one. `ilold_use` can be called any number of times in the same session to switch back and forth between programs.

## Example session

A natural-language prompt for an MCP-aware client looks like this:

> Audit the `staking` program. Look for paths where the admin signer check can be bypassed. Create a user `alice`, run `stake` for 1000 lamports, and produce a coverage report at the end.

The client typically resolves it as the following tool sequence:

1. `ilold_funcs_all` to enumerate instructions and admin-gating hints.
2. `ilold_info` on each instruction the model wants to inspect.
3. `ilold_users_new` to create `alice`.
4. `ilold_call` for `initialize_pool` and then `stake`.
5. `ilold_coverage` to read aggregated runtime metrics.
6. `ilold_finding` if the model identifies an issue, followed by `ilold_export`.

Every step also fires a WebSocket patch from `ilold serve`, so a browser tab pointed at the canvas reflects the graph evolving in real time.

## Limitations

- **Single active program at a time.** The handler tracks one active program. Call `ilold_use <program>` to switch — the MCP subprocess does not need to be restarted to point at a different program in the same workspace.
- **Static tool registry.** Tools are derived from `SOLANA_HELP_BLOCKS` once at startup. Reloading the backend project does not change the tool set; only the data behind the tools.
- **No sandbox over the LLM.** Every tool that mutates the VM (`ilold_call`, `ilold_clear`, `ilold_back`, `ilold_scenario`) is invocable without confirmation from the server. Sandboxing is delegated to the MCP client: mature clients prompt the human before destructive tools (those whose names contain `clear`, `delete`, `reset`).
- **Narration is best-effort.** `--narration` emits a `notifications/progress` message keyed by the request `progressToken`. Clients that do not declare a progress token in the request silently drop the notification.
- **stdio only.** SSE and streamable HTTP transports are out of scope for v1. Every supported client uses stdio.

## Troubleshooting

| Symptom | Likely cause |
| --- | --- |
| `Cannot reach Ilold server at <url>` on startup | `ilold serve` is not running, or `--server-url` points to the wrong port. |
| Tools do not appear in the client | The client could not spawn `ilold`. Check that the binary is on `PATH` or use an absolute path in `command`. Inspect the client log (`~/Library/Logs/Claude/mcp-server-ilold.log` for Claude Desktop on macOS). |
| `No active contract` from every tool but `ilold_programs` | The session has no active program. Call `ilold_use <program>` (or restart the subprocess with `--contract <name>`). |
| Tool call returns `Error: ...` | The backend rejected the `SolanaCommand`. The error text is the same as the REPL would print; check the active program (`ilold_programs`) and the instruction arguments. |
