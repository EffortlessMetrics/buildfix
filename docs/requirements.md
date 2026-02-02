# buildfix — Requirements

## Purpose

`buildfix` is the ecosystem **actuator**. It consumes sensor receipts and emits a **plan** (and optionally applies it) consisting of **mechanical, allowlisted edits** that reduce toil.

It answers:

> “Given these findings, can we safely apply deterministic repo changes?”

It does **not** prove correctness. Sensors + CI are the proof layer.

## Truth layer

**Actuation.** `buildfix` is the only tool in the ecosystem permitted to write to the repo.

## Non-goals (hard boundaries)

`buildfix` MUST NOT:

- run builds/tests/coverage/benchmarks
- call the network
- run arbitrary shell commands as part of fixes
- refactor code
- make style-only/format-only edits
- invent dependency versions without a repo-local source of truth
- touch `Cargo.lock` by default
- modify files outside allowlisted targets for selected fixers
- apply unsafe or guarded changes without explicit opt-in
- “integrate” by linking against sensor libraries across repos

## Inputs

### Primary input: receipts

- Receipts from `artifacts/**/report.json` (envelope v1).
- `buildfix` MUST rely only on the envelope fields + finding identity:
  - `tool.name` (sensor id)
  - `finding.check_id` (producer, when present)
  - `finding.code` (classification)
  - `finding.location.path` (best-effort)
  - `finding.data` (optional *hints*, never required)

### Secondary input: repo files

Only files needed for deterministic edits, for example:

- root `Cargo.toml`
- member `Cargo.toml` files (as needed by a fix)
- `rust-toolchain.toml` (only for allowlisted toolchain/MSRV fixes)
- other config files only when a fix explicitly requires them

### Policy input (user / repo)

- allow/deny list for fix keys (and/or check/code patterns)
- safety policy knobs:
  - allow guarded ops
  - allow unsafe ops only with explicit parameters
- operational knobs:
  - require clean working tree (default)
  - max operations / max files / max diff size
  - backup strategy

## Outputs

### Canonical artifacts

`buildfix` MUST produce:

- `artifacts/buildfix/plan.json`  (`buildfix.plan.v1`)
- `artifacts/buildfix/plan.md`    (human-readable summary)
- `artifacts/buildfix/patch.diff` (unified diff preview)
- `artifacts/buildfix/apply.json` (`buildfix.apply.v1`) **when applying**

### Optional cockpit-facing receipt

- `artifacts/buildfix/report.json` (`buildfix.report.v1`, envelope compatible)

This allows the director/cockpit to display “Fix plan available / applied / blocked” without special casing.

## CLI surface (stable)

- `buildfix plan`
- `buildfix apply`
- `buildfix list-fixes`
- `buildfix explain <fix-key|check_id|code>`

## Exit semantics

- `0` success (plan created; apply completed; or “nothing to do”)
- `2` policy block (unsafe/guarded not allowed, allowlist/denylist, precondition mismatch, dirty tree)
- `1` tool/runtime error (I/O, parse errors, invalid receipts)

## Safety model (enforced)

Every operation is classified:

- **safe**: fully determined from repo truth, no ambiguity
- **guarded**: deterministic but high impact; requires explicit opt-in (`--allow-guarded`)
- **unsafe**: requires user parameters; plan-only unless parameters supplied

Defaults:

- Plan includes all candidates, marking blocked ones explicitly.
- Apply refuses guarded unless explicitly allowed.
- Apply refuses unsafe unless required parameters are present.
- Apply refuses dirty working tree unless `--allow-dirty` is set.

## Determinism + auditability

`buildfix` MUST be deterministic:

- Same receipts + same repo state → byte-stable `plan.json` and `patch.diff`.
- Operations apply in deterministic order.
- Plan contains a precondition snapshot (file digests + best-effort git identity).
- Apply verifies preconditions and refuses if the repo changed.
- Apply always produces an audit trail (patch + apply.json).

## v0.1 fix set (allowlisted)

Only fixes that are provably deterministic from repo-local truth:

1. Workspace resolver v2 (`[workspace].resolver = "2"`)
2. Path dependency requires version (read target crate version)
3. Workspace dependency inheritance normalization (preserve flags)
4. MSRV normalization when a single declared source-of-truth exists

Everything else is deferred.

## Definition of done (v0.1)

- Plan/apply artifacts + schema validation in CI
- At least 4 fixers (above) with golden fixtures
- BDD feature coverage for plan/apply + safety gates
- Proptest on TOML editing invariants (semantic preservation)
- Fuzz targets for receipt parsing and TOML transforms
- Mutation testing on domain policy/planner logic
