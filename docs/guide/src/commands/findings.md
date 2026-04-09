# Findings Commands

Findings commands let you record security observations during an audit session. Findings are tied to the current session state and can be exported as a markdown report.

## finding

`fi [severity] [title]` or `finding [severity] [title]`

Records a security finding. Can be used in two modes:

**Inline mode** -- pass severity and title directly:

```
ilold[→ deposit → withdraw]> fi high Reentrancy in withdraw before balance update

  ✓ Finding F-001 added
```

**Interactive mode** -- run `fi` with no arguments to be prompted:

```
ilold[→ deposit → withdraw]> fi
  Severity (critical/high/medium/low/info):
  > high
  Title:
  > Reentrancy in withdraw before balance update
  Description (optional):
  > The external call on L38 occurs before totalStaked is decremented.
  ✓ Finding F-001 added
```

Valid severities: `critical`, `high`, `medium`, `low`, `info` (or `informational`).

The finding captures the current session sequence automatically.

## note

`n <text>` or `note <text>`

Attaches a free-text note to the current session step. Notes are included in the exported report.

```
ilold[→ deposit → withdraw]> n Check if msg.value can be zero here

  ✓ Note added
```

## scenario

`sc <name>` or `scenario <name>`

Names the current call sequence. Run without arguments to see the current name.

```
ilold[→ deposit → withdraw]> sc reentrancy-attack

  Scenario: reentrancy-attack
```

```
ilold[→ deposit → withdraw]> sc

  Current scenario: reentrancy-attack
```

## status

`status <function> <status>`

Sets the review status for a function. Useful for tracking audit progress.

```
ilold[Staking]> status deposit reviewed

  ✓ Status updated
```

Valid statuses: `reviewed`, `suspicious`, `vulnerable`, `clean`, `inprogress`, `notreviewed`.

## findings

`fl` or `findings`

Lists the count of recorded findings. Use [export](#export) to see full details.

```
ilold[Staking]> fl

  2 finding(s) recorded. Use export to export.
```

## export

`ex` or `export`

Exports all findings, notes, and status changes as a markdown report. The file is written to the current directory.

```
ilold[Staking]> ex

  ✓ Exported to ilold-report-Staking.md
```
