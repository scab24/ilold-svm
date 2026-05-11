# WebSocket Events

The `/ws` route emits `ServerMessage` events (JSON, tagged on a `type` field) whenever the active session changes. The full mapping from internal `CanvasPatch` variants to wire events lives in `crates/ilold-web/src/ws/handler.rs`.

## Session events

| Event | Fields | Trigger |
| --- | --- | --- |
| `session_add_node` | `scenario`, `function`, `access`, `step_index`, optional `runtime` (CU + diffs + logs excerpt on Solana) | `call` adds a step |
| `session_remove_node` | `scenario` | `back` rewinds the last step |
| `session_clear` | `scenario` | `clear` wipes the scenario |
| `session_highlight` | `scenario`, `function` | Auditor selects a step (web canvas) |

## Scenario events

| Event | Fields | Trigger |
| --- | --- | --- |
| `scenario_created` | `name` | `scenario new` |
| `scenario_switched` | `from`, `to` | `scenario switch` |
| `scenario_deleted` | `name` | `scenario delete` |
| `scenario_forked` | `from`, `to`, `at_step` | `scenario fork` |
| `scenario_store_reloaded` | `active` | After `load`, when the entire scenario tree is rehydrated |

## Solana-only events

| Event | Fields | Trigger |
| --- | --- | --- |
| `solana_users_changed` | `scenario` | `users new`, `airdrop`, or anything that mutates the keypair set |
| `session_overlay_update` | `scenario`, `ix_name`, `calls_added`, `failed_added`, optional `cu`, `cpi_targets_added` | Runtime overlay aggregates updated after a `call` |

## Client → server

The only message the client can send is a `search` query consumed by `crates/ilold-web/src/ws/search.rs`. Responses come back as `search_result` (one per match) and `search_complete` (`total` count).

## PTY bridge

`GET /ws/pty` opens a PTY for the embedded REPL in the web canvas. The protocol is binary-passthrough; the wire format is documented inline in `crates/ilold-web/src/ws/pty.rs`.

## Related pages

- [HTTP API](./api-endpoints.md)
- [Solana REPL: Scenarios](../solana/repl/scenarios.md): the source of the scenario events.
- [Solidity REPL: Scenarios](../solidity/repl/scenarios.md): same events on the Solidity side.
