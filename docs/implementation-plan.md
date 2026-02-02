# buildfix — Implementation Plan

This plan assumes:
- receipts-first ecosystem (envelope ABI)
- microcrate workspace
- test-heavy posture (BDD + golden fixtures + proptest + fuzz + mutation)

## Phase 0 — Contracts and skeleton (weekend-sized, but foundational)
Deliverables:
- schemas committed under `schemas/`
- protocol DTOs in `buildfix-protocol`
- minimal CLI:
  - `buildfix plan` writes an empty plan when no candidates exist
  - `buildfix apply` refuses without a plan
- CI:
  - schema validation for plan/apply/report artifacts
  - golden test harness skeleton

Tasks:
- Define schema IDs + jsonschema files:
  - buildfix.plan.v1
  - buildfix.apply.v1
  - buildfix.report.v1 (optional)
- Create xtask:
  - `xtask schema` (regen + verify)
  - `xtask fixtures` (create/update fixture repos)
- Create artifact writer utilities and canonical paths.
- Establish stable ordering helpers.

Acceptance:
- Running `buildfix plan` on an empty fixture produces byte-stable artifacts.

## Phase 1 — Receipt ingestion and normalization (read-only core)
Deliverables:
- Receipt discovery and parsing
- NormalizedFinding + FixKey extraction
- Allow/deny matching + safety gating model
- Plan JSON + Plan MD skeleton

Tasks:
- Implement receipt ingestion:
  - scan artifacts
  - parse JSON
  - tolerate missing optional fields
  - normalize paths
- Implement policy parsing from `buildfix.toml`.
- Implement planner skeleton:
  - findings → candidates (stub fixers)
  - deterministic ordering
  - blocked ops recorded explicitly

Tests:
- BDD: “plan empty”, “denied fix is blocked”, “allowlist restricts fixes”
- Golden: plan.json stable regardless of receipt discovery order
- Fuzz: receipt parse never panics

## Phase 2 — Editing engine and patch preview
Deliverables:
- TOML editing ops (TomlSet/TomlRemove)
- precondition snapshot and verification
- patch preview generation
- apply engine with backups and atomic writes

Tasks:
- Implement RepoPort adapter:
  - backups (suffix strategy)
  - atomic writes (temp + rename)
- Implement precondition snapshot:
  - sha256 per file
  - best-effort head sha
  - dirty flag
- Implement apply flow:
  - verify preconditions
  - apply ops deterministically
  - emit apply.json

Tests:
- BDD: apply blocked on precondition mismatch, no writes
- Proptest: atomic write preserves file content integrity on failure paths

## Phase 3 — Fixers (v0.1 allowlist)
Each fixer must ship with:
- at least one BDD scenario
- golden plan/apply outputs
- proptest invariants where semantic preservation matters

### Fixer A: workspace resolver v2
- Find from builddiag receipt (or detect directly as plan-time validation)
- Edit root Cargo.toml `[workspace].resolver = "2"`
- Safety: safe

### Fixer B: depguard path dep requires version
- Resolve target crate version from repo-local `Cargo.toml`
- If missing/ambiguous → unsafe, blocked unless user provides param
- Safety: safe/unsafe depending on determinism

### Fixer C: workspace inheritance normalization
- Replace member dep entries with `{ workspace = true }` while preserving flags
- Conflicts → guarded by default
- Safety: safe/guarded

### Fixer D: MSRV normalization to workspace standard
- Only safe if workspace standard exists
- Otherwise unsafe

## Phase 4 — buildfix.report.v1 envelope output (cockpit integration)
Deliverables:
- buildfix emits an envelope-compatible report summarizing plan/apply results
- stable internal codes for buildfix itself
- `buildfix explain` registry for fix keys and internal codes

Tests:
- golden report.json output for each fixture
- conformance: report validates against schema

## Phase 5 — Hardening and release discipline
Deliverables:
- fuzz targets:
  - receipt parser
  - TOML transform
- mutation testing in scheduled CI (domain crates only)
- cross-platform build matrix
- packaging includes schemas and templates in cargo package

Acceptance:
- No panics under fuzz
- Mutation tests catch trivial regressions in safety logic
- Release binaries published and installable via workflow

## Phase 6 — Optional convenience (post-v0.1)
Only add if demanded:
- apply-from-plan with optional auto-commit (maintainer-only)
- more op types (anchored text replace) with strict constraints
- support for additional file types beyond TOML when they are provably mechanical
