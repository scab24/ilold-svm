# WebSocket Events

The `/ws` route emits `ServerMessage` events (JSON, tagged on a `type` field) whenever the active session changes. The full mapping from internal `CanvasPatch` variants to wire events lives in `crates/ilold-web/src/ws/handler.rs`.

## Session events

| Event | Fields | Trigger |
| --- | --- | --- |
| `session_add_node` | `scenario`, `function`, `access`, `step_index`, optional `runtime` (CU + diffs + logs excerpt) | `call` adds a step |
| `session_remove_node` | `scenario` | `back` rewinds the last step |
| `session_clear` | `scenario` | `clear` wipes the scenario |
| `session_highlight` | `scenario`, `function` | Auditor selects a step from the web canvas |

## Scenario events

| Event | Fields | Trigger |
| --- | --- | --- |
| `scenario_created` | `name` | `scenario new` |
| `scenario_switched` | `from`, `to` | `scenario switch` |
| `scenario_deleted` | `name` | `scenario delete` |
| `scenario_forked` | `from`, `to`, `at_step` | `scenario fork` |
| `scenario_store_reloaded` | `active` | After `load`, when the entire scenario tree is rehydrated |

## Runtime events

| Event | Fields | Trigger |
| --- | --- | --- |
| `solana_users_changed` | `scenario` | `users new`, `airdrop`, or anything that mutates the keypair set |
| `session_overlay_update` | `scenario`, `ix_name`, `calls_added`, `failed_added`, optional `cu`, `cpi_targets_added` | Runtime overlay aggregates updated after a `call` |

## Client → server

The current `/ws` socket is server-push only: clients receive events, the only thing they may send back is a `Close` frame. Programmatic interaction with the backend goes through `POST /api/cmd`.

## PTY bridge

`GET /ws/pty` opens a PTY for the embedded REPL in the web canvas. The protocol is binary passthrough; the wire format is documented inline in `crates/ilold-web/src/ws/pty.rs`.

## Related pages

- [HTTP API](./api-endpoints.md)
- [Scenarios](../solana/repl/scenarios.md): the source of the scenario events.
