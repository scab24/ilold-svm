# HTTP API Reference

ilold exposes an HTTP API on the configured port (default `8080`). All endpoints return JSON. The routes are defined in `crates/ilold-web/src/lib.rs::build_router`.

## Command bus

### POST /api/cmd

Execute a session command against the active program and scenario. This is the single entry point for every state-mutating session operation.

**Request body:**

```json
{
  "contract": "staking",
  "command": { "Call": { "ix": "stake", "args": { "amount": 1000 }, "accounts": { "pool": "pool", "user_stake": "alice_stake", "user": "alice" }, "signers": ["alice"] } }
}
```

`contract` is the program name. It defaults to the first program in the workspace when omitted.

`command` carries a `SolanaCommand` variant (see `crates/ilold-solana-core/src/exploration/commands.rs`). Headline variants:

| Variant | Payload | Description |
| --- | --- | --- |
| `Call` | `{ ix, args, accounts, signers }` | Run an instruction against the LiteSVM and append a step |
| `Back` | `"Back"` | Remove the last step and rewind the VM |
| `Clear` | `"Clear"` | Reset scenario steps and the underlying VM |
| `Funcs` / `Vars` / `Info` / `Coupling` / `Coverage` | metadata | Inspect the typed model and runtime aggregates |
| `State` / `Session` / `Step` / `Timeline` / `Inspect` | session queries | Decoded views of the active scenario |
| `Users` / `UsersNew` / `Airdrop` / `TimeWarp` / `Pda` | runtime | LiteSVM controls |
| `Scenario` | `{ sub: New | List | Switch | Fork | Delete }` | Scenario management |
| `Finding` / `Note` / `Status` / `Findings` / `Export` | deliverable | Audit journal and Markdown export |
| `SaveSession` / `LoadSession` | persistence | JSON scenario store under `~/.ilold/sessions/` |

The response is a `SolanaCommandResult` variant (`StepAdded`, `CallFailed`, `StateView`, `Timeline`, `Coverage`, `Error`, …).

## Project endpoints

| Endpoint | Description |
| --- | --- |
| `GET /api/project` | Project summary: `kind: "solana"`, list of programs with instruction + account-type counts |
| `GET /api/project/map` | Full project map consumed by the web canvas (programs, instructions, account types) |

## Program endpoints

| Endpoint | Description |
| --- | --- |
| `GET /api/program/{name}/view` | Full `ProgramView` for the named program: instructions (typed args, accounts with flags, signers, PDAs, admin-gated flag, coupling hints), account types, discriminators |
| `GET /api/program/{name}/overlay` | Runtime overlay aggregated over the active scenario: calls-per-instruction, failures, CU stats, CPI edges (`?scenario=<name>` overrides the default) |
| `GET /api/program/{name}/{ix}/source` | Anchor handler source slice from `programs/<program>/src/lib.rs`, with `file_path`, `source`, and `span` (line/column range) |

## Users + scenarios

| Endpoint | Description |
| --- | --- |
| `GET /api/users/{scenario}/labels` | Returns the keypair labels (pubkey → name) for the given scenario; used by the canvas to render `users new <name>` aliases on signer/payer pubkeys |
| `GET /api/scenarios` | Scenario list for the active program (name, active flag, step count) |
| `GET /api/scenarios/all` | Scenarios with their step lists across every program in the workspace |

## Session step trace

### GET /api/session/step/{index}/trace

Returns the persisted runtime trace of a session step (logs, CU, inner instructions). 404 if the step has no trace recorded.

## Annotations

| Endpoint | Description |
| --- | --- |
| `GET /api/annotations` | List all canvas annotations |
| `POST /api/annotations` | Create an annotation |
| `PUT /api/annotations/{id}` | Update an annotation |
| `DELETE /api/annotations/{id}` | Delete an annotation |

## WebSocket

`GET /ws` upgrades to a WebSocket connection. See [WebSocket events](./websocket.md) for the full event vocabulary and payload shapes.

`GET /ws/pty` provides a PTY bridge used by the embedded REPL in the web canvas. Binary passthrough; the wire format is documented inline in `crates/ilold-web/src/ws/pty.rs`.

## Related pages

- [WebSocket events](./websocket.md)
- [MCP server](./mcp.md)
- [REPL: Session](../solana/repl/session.md)
- [Known Limitations](./limitations.md)
