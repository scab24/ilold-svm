# Workspace Commands

Workspace commands handle session persistence and external tools.

## save

`save <name>`

Saves the current session (steps, findings, notes, statuses) to a JSON file under `~/.ilold/sessions/`.

```
ilold[→ deposit → withdraw]> save staking-audit

  ✓ Saved to /Users/you/.ilold/sessions/staking-audit.json
```

You can resume this session later with [load](#load), even across different ilold runs, as long as the same contract files are loaded.

## load

`load <name>`

Loads a previously saved session from `~/.ilold/sessions/`.

```
ilold[Staking]> load staking-audit

  ✓ Session loaded (2 steps)
```

The prompt updates to reflect the loaded steps. The session replaces whatever is currently in memory.

## browser

`browser`

Prints the base URL of the HTTP API the REPL is talking to. `explore` runs the API in-process by default; pass `--attach <url>` to point the REPL at a separate `serve` instance instead.

```
ilold[Staking]> browser

  API running at http://127.0.0.1:52431/api/
```

The web canvas (when running `serve` and opening the URL in a browser) subscribes to the same HTTP/WS endpoints. See [HTTP API Reference](../../reference/api-endpoints.md) for the full surface.

## quit

`q`, `quit`, or `exit`

Exits the REPL. `Ctrl+D` and `Ctrl+C` also work.

Unsaved session data is lost on exit. Use [save](#save) before quitting if the session needs to survive.
