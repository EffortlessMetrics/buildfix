# buildfix — Architecture

This is the repo-level architecture document for `buildfix`.

## Role
`buildfix` is the ecosystem actuator. It consumes receipts and produces a plan (and optionally applies it) of allowlisted, mechanical edits.

## Truth layer
Actuation. `buildfix` is the only writer.

## Inputs
- Receipts: `artifacts/**/report.json`
- Repo files: only those required by selected fixers
- Config: `buildfix.toml` (allow/deny, safety, caps)

## Outputs
Canonical:
- `artifacts/buildfix/plan.json` (buildfix.plan.v1)
- `artifacts/buildfix/plan.md`
- `artifacts/buildfix/patch.diff`
- `artifacts/buildfix/apply.json` (buildfix.apply.v1)

Optional:
- `artifacts/buildfix/report.json` (buildfix.report.v1; cockpit-friendly)

## Non-goals
See docs/requirements.md (hard boundaries).

## Architectural style
Hexagonal / clean:
- domain core is pure logic
- adapters handle filesystem/git/schema/diff
- CLI is wiring and exit mapping

## Workspace structure (microcrates)

crates/
  buildfix-protocol/
  buildfix-receipts/
  buildfix-domain/
  buildfix-edit/
  buildfix-render/
  buildfix-cli/
xtask/
schemas/
tests/

Publishing:
- publish only the CLI crate (`buildfix` binary) by default.
- internal crates are workspace-only (`publish = false`) unless a real external embedder exists.

## Ports
Domain depends on ports, not implementations:
- ReceiptPort (discover/read receipts)
- RepoPort (read/write/backup/atomic)
- RepoIdentityPort (best-effort git identity + dirty flag)
- ClockPort
- SchemaPort (optional)
- DiffPort (optional; used for patch previews)

## Determinism guarantees
- stable receipt discovery order
- stable candidate ordering
- stable op ordering (file → kind → toml_path → id)
- stable rendering (plan.md and patch.diff)
- stable truncation behavior (caps are explicit and recorded)

## Safety enforcement
- plan is read-only
- apply requires explicit opt-in and enforces safety gates
- precondition mismatch blocks (policy, exit 2)
- backups are written before any modifications

## Integration into cockpit
- buildfix is typically run as:
  - plan-only in PRs (produce patch artifact)
  - apply in maintainer-only workflows
- cockpit may ingest buildfix.report.v1 to display plan/apply status.
