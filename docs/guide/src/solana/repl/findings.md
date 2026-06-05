# Findings Commands

Findings, notes, and per-instruction status flags are recorded against the active scenario and aggregated by `export`. The Solana export adds runtime metadata (CU, logs, account diffs) to each step in the report.

## finding

`fi <severity> <title>` or `finding <severity> <title>` (alias: `fi`)

Records a security finding tied to the latest step of the active scenario.

Flags:

| Flag | Description |
| --- | --- |
| `--rec="..."` | Optional remediation recommendation. Quote it if it contains spaces. |

Valid severities: `critical`, `high`, `medium`, `low`, `info`.

```
ilold[staking → … → stake]> fi high reentrancy via stake
  ✓ finding F-001
```

```
ilold[staking → … → claim_rewards]> finding critical missing signer --rec="require admin signature"
  ✓ finding F-002
```

**Returns:** `FindingAdded { id }`.

## findings

`fl` or `findings`

Lists every finding recorded in the active scenario, with severity, title, the step it is attached to, and the optional remediation.

```
ilold[staking]> fl
  F-001 high [2026-05-09T10:12:00Z] reentrancy via stake
  F-002 critical [2026-05-09T10:14:00Z] missing signer
    require admin signature
```

**Returns:** `FindingsList { items: [{ id, severity, title, description, created_at }] }`.

## note

`n <text>` or `note <text>`

Attaches a free-form annotation to the active scenario. Notes are stored alongside findings and surface in the exported report.

```
ilold[staking → … → stake]> n suspicious admin path here
  ✓ note recorded
```

**Returns:** `NoteAdded`.

## status

`status <ix> <open | reviewed | finding>`

Sets the review status of an instruction. Useful for tracking audit progress.

```
ilold[staking]> status stake reviewed
  ✓ status updated
ilold[staking]> status claim_rewards finding
  ✓ status updated
```

Note: the supported statuses are `open`, `reviewed`, `finding` (alias `found`).

**Returns:** `StatusUpdated`.

## export

`ex` or `export`

Generates a Markdown deliverable aggregating audit metadata, severity matrix, methodology, findings (with step index, recommendation, and runtime metadata) and per-scenario step lists across **all** scenarios.

Flags:

| Flag | Description |
| --- | --- |
| `--auditor=<name>` | Auditor identity in the report metadata |
| `--version=<v>` | Project version pinned in the report |
| `--date=<YYYY-MM-DD>` | Audit date override (defaults to today) |

```
ilold[staking]> export
  ✓ markdown report (4321 bytes)

  # ilold audit report
  ...

ilold[staking]> export --auditor="Alba S." --version=v1.2 --date=2026-05-09
  ✓ markdown report (4567 bytes)
```

**Returns:** `Exported { markdown, bytes }`. The CLI prints the full Markdown body after the header line.

## Notes

- Findings are scoped to the scenario they were recorded in but the export merges all of them.
