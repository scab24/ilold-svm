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

Opens the web UI. ilold starts an API server on startup; `browser` provides the URL.

```
ilold[Staking]> browser

  API running at http://127.0.0.1:52431/api/
```

## quit

`q`, `quit`, or `exit`

Exits the REPL. You can also press `Ctrl+D` or `Ctrl+C`.

Unsaved session data is lost on exit. Use [save](#save) before quitting if you want to resume later.
