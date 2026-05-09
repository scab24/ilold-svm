# SDD roadmap — what we ship next, and why

We close the Solana-parity sprint with 10 commits and a full smoke suite
green. Three pieces of debt remain that the auditor and developer
audits flagged. This roadmap explains, for each one, the business
reason, the proposed scope, the risk of *not* doing it, and the SDD
artifacts we will produce before touching code.

## Why SDD here

The Solana parity sprint moved fast: 10 audit rounds, ~21 fixes, a
scenario suite. From here forward the changes are smaller in line
count but bigger in surface area (CI policy, customer deliverable
format, security-sensitive persistence). SDD (proposal → spec / design
→ tasks → apply → verify → archive) protects us from:

- Shipping a CI policy nobody reviewed.
- Producing a deliverable export that misleads clients.
- Persisting auditor keypairs without thinking through the threat model.

Each item below produces its artifacts under `docs/sdd/<change>/`. That
directory is gitignored on purpose (it is internal scratch); only the
final code change and the commit body land in version control. The
contents of this roadmap are the public-facing summary.

## Topic 1 — `sdd/01-ci-pipeline` (smallest, ships first)

**Goal**: every PR that touches the workspace must run `cargo test
--workspace` and `bash tests/scenarios/run.sh`. Without it, the next
contributor (human or LLM) can break T-R33/T-R37/T-R40 silently.

**Justification**: audit round 8 found `tests/e2e_lever.rs` already
broken on `main` because the function signature changed and nobody
caught it. CI is the cheapest hedge against that.

**Scope**:
- `.github/workflows/test.yml` runs the unit tests + the scenario
  suite on Linux runners.
- Skip the python WS scenario when `pip install websockets` is not
  available, so a minimal CI image still works.
- No external infra (no Docker, no caching tweaks beyond cargo's
  default).

**Out of scope**: building/serving the Anchor `.so` from scratch (the
fixture ships pre-compiled), Solidity tests (already covered in unit
tests), publish/release pipelines.

**SDD artifacts**: `docs/sdd/01-ci-pipeline/{proposal,spec,design,tasks}.md`.

**Apply estimate**: 30 minutes.

## Topic 2 — `sdd/02-audit-deliverable-export` (next)

**Goal**: `execute_export` produces a markdown document a security
auditor can hand to a client, not the current minimalist debug dump.

**Justification**: round 9 (auditor UX) flagged the export as a
blocker for delivering paid audits — it lacks audit metadata
(date, auditor name, contract version), methodology section,
per-finding code location / reproduction / impact / recommendation,
and severity matrix.

**Scope**:
- `SolanaCommand::Export { audit_metadata: Option<AuditMetadata> }`
  with an explicit struct (auditor name, project, commit hash,
  date).
- New sections in the markdown: Methodology, Severity Matrix,
  Per-finding template (Title / Severity / Location / Reproduction /
  Impact / Recommendation), Coverage (scenarios + steps explored).
- A `Finding` model extended with `affected_step_index`,
  `affected_account` so the per-finding location is concrete.
- Bilingual hooks: keep markdown rendering pure, easy to swap to
  HTML / PDF later.

**Out of scope**: PDF rendering, tracking links to remediation PRs,
multi-program reports.

**SDD artifacts**: `docs/sdd/02-audit-deliverable-export/{proposal,spec,design,tasks}.md`.

**Apply estimate**: 2–3 hours.

## Topic 3 — `sdd/03-save-load-deterministic` (highest risk, last)

**Goal**: `Save → Load` reproduces PDAs, signatures and balances
exactly. Today `LoadSession` re-creates user keypairs with `Keypair::new()`,
so any program that derives PDAs from a user pubkey or validates
`require!(account.owner == previous_signer)` breaks under reload.

**Justification**: audit round 9 raised this as the deal-breaker for
reproducing findings the next morning. Without it the scenario suite
itself can drift.

**Scope (security implications, hence SDD)**:
- Persist `users` keypairs in the JSON. Default opt-out: encrypted
  with a passphrase derived from the auditor's input. Plaintext
  available behind a `--insecure-save` flag for local-only sessions.
- Verify replay against persisted pubkeys, not regenerated ones.
- Emit a versioned JSON header (`session-format: "2"`) so older saves
  keep loading via the existing best-effort path.

**Threat model questions for the spec**:
- Where do encryption keys live? Local keystore? KDF on prompt?
- What happens if a saved session is committed to git by mistake?
- Test programs vs production programs: do we want to *forbid*
  saving with mainnet program IDs?

**Out of scope**: hardware-wallet integration, multi-user shared
sessions.

**SDD artifacts**: `docs/sdd/03-save-load-deterministic/{proposal,spec,design,tasks}.md`.

**Apply estimate**: 3–5 hours, with a security review pass before
shipping.

## Order, gating, parallelism

We ship serially: 1 → 2 → 3. Reasons:

- Topic 1 (CI) protects the work in 2 and 3.
- Topic 2 changes data shapes that topic 3 needs.
- Topic 3 requires careful threat-model review and is the last to
  ship, so it inherits the safety net of CI + a richer Finding model.

Each topic exits `apply` only when the matching scenario test (or a
new one in `tests/scenarios/`) is green.

## What we are NOT doing in this roadmap

- `slice` / `trace` / `sequence` for Solana (`Phase 2` backlog —
  needs anchor-syn AST extractor + per-instruction CFG).
- CFG visual redesign (waiting on user screenshot).
- Cross-program CPI modeling (T-R16).
- TUI ratatui Solana mode.

These are documented in `solana-support.md` and the engram timeline so
the next contributor finds them quickly.
