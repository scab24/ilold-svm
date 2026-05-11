# Workspace Commands

Workspace commands handle persistence and external tooling. On Solana, `save` / `load` cover not just the step list but the entire scenario tree, including runtime traces, findings, fork origins, and the original call payloads needed to replay each call against a fresh VM.

## save

`save <name>`

Serialises the active session to `~/.ilold/sessions/<name>.json`.

Flags:

| Flag | Description |
| --- | --- |
| `--with-keypairs` | Bundle plaintext test keypairs for deterministic reload. Do NOT commit the resulting file. |

```
ilold[staking]> save reentrancy-attack
  ✓ session JSON (8421 bytes)

ilold[staking]> save reentrancy-attack --with-keypairs
  ✓ session JSON (9532 bytes)
```

**Returns:** `SessionSaved { json }`. The CLI prints the JSON byte count. The file is written by the dispatch layer; warnings about bundled keypairs come from the surrounding CLI flow, not the result variant itself.

Without `--with-keypairs`, `load` regenerates user keypairs and any PDA derived from a signer pubkey will resolve to different addresses on reload. With the flag, the JSON embeds the keypairs in plaintext so the next `load` reproduces the exact same pubkeys.

## load

`load <name>`

Reads `~/.ilold/sessions/<name>.json`, boots a fresh VM per scenario, re-airdrops the in-memory users, and replays each call from the persisted payload.

```
ilold[staking]> load reentrancy-attack
  ✓ loaded program=staking steps=3
```

**Returns:** `SessionLoaded { program, steps }` where `steps` is the list of instruction names that were replayed.

The reconstructed VM state matches the saved snapshot for typical Anchor flows. Programs with non-deterministic behaviour or balances above the default replay cap may diverge; see [Solana: Limitations](../limitations.md).

## browser

`browser`

Prints the base URL of the local HTTP API. The web canvas (`crates/ilold-web/frontend`) subscribes to the same `/api` and `/ws` endpoints, so anything the REPL runs surfaces there live.

```
ilold[staking]> browser
  · Web UI not yet available in explore mode.
  · API running at http://127.0.0.1:8080/api/
```

See [HTTP API Reference](../../reference/api-endpoints.md) for the full surface and [WebSocket events](../../reference/websocket.md) for the live update stream.

## quit

`q`, `quit`, or `exit`

Exits the REPL. `Ctrl+D` and `Ctrl+C` also work.

Unsaved scenarios are lost on exit. Use `save` before quitting if the session needs to survive.
