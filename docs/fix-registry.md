# buildfix — Fix Registry

This document defines the initial fix registry: which finding keys buildfix can respond to, and what it will do.

## Fix routing key

Preferred key:
- `(sensor_id, check_id, code)`

Fallback:
- `(sensor_id, code)` when check_id is missing

In practice, `sensor_id` should match receipt `tool.name`.

## Registry entries (v0.1)

### 1) Workspace resolver v2
- Fix key: `builddiag / workspace.resolver_v2 / missing_or_wrong`
- Safety: safe
- Target: root `Cargo.toml`
- Edit: ensure `[workspace].resolver = "2"`

Notes:
- If repo is not a workspace (no `[workspace]`), fixer is inapplicable.
- If file is unparseable TOML, tool error.

### 2) Path dependency requires version
- Fix key: `depguard / deps.path_requires_version / missing_version`
- Safety: safe (when target version is readable); unsafe otherwise
- Target: manifest containing the dependency
- Edit: set `version = "<target crate version>"` for `{ path = "...", ... }`

Preconditions:
- The dependency target `Cargo.toml` exists and has `package.version`.
- If multiple targets match, or version missing, mark unsafe and block unless user supplies `--param version=...`.

### 3) Workspace dependency inheritance normalization
- Fix key: `depguard / deps.workspace_inheritance / not_inherited`
- Safety: safe (single source-of-truth exists and keys preserved), otherwise guarded
- Target: member `Cargo.toml`
- Edit: replace member dep spec with `{ workspace = true }` while preserving allowed keys:
  - `features`, `optional`, `default-features`, `package`, `registry` (if relevant)

Guarded cases:
- Member has an explicit version that conflicts with workspace dep.
- Workspace dep is complex (git/path) and member has additional overrides.

### 4) MSRV normalization to workspace standard
- Fix key: `builddiag / rust.msrv_consistent / mismatch`
- Safety: safe only if `[workspace.package].rust-version` exists; unsafe otherwise
- Targets: member manifests with drifting `rust-version` fields
- Edit: set member `package.rust-version` to workspace value (or remove if policy says “inherit”)

Unsafe cases:
- No declared workspace standard
- Multiple competing “standards” in the workspace

### 5) Unused dependency removal
- Fix key: `cargo-machete / deps.unused_dependency / *` (also `cargo-udeps`, `udeps`, `machete`)
- Safety: unsafe
- Target: dependency entry in member `Cargo.toml`
- Edit: remove reported dependency key via `toml_remove`

Unsafe rationale:
- Sensor coverage can miss feature/platform-specific usage
- Manual confirmation is required before apply (`--allow-unsafe`)

### 6) License normalization
- Fix key: `cargo-deny / licenses.unlicensed / *` (also `deny`)
- Safety: guarded (unsafe when no canonical workspace license exists)
- Target: crate `Cargo.toml` reported by cargo-deny finding
- Edit: set `package.license` to workspace canonical license

Canonical source-of-truth:
- `[workspace.package].license` in root `Cargo.toml`
- fallback: `[package].license` in root `Cargo.toml`

Unsafe fallback:
- If no canonical license is declared, operation requires `--param license=...`.

## buildfix-internal codes

These are buildfix’s own finding codes (for buildfix.report.v1):

- `buildfix.plan.blocked`
- `buildfix.plan.empty`
- `buildfix.apply.failed`
- `buildfix.apply.blocked_preconditions`
- `buildfix.policy.denied`
- `tool.runtime_error` (shared across ecosystem)

## Compatibility note

This registry is intentionally narrow. New fixes are added only when they are:
- deterministic
- explainable
- reversible
- safe by default

If a proposed fix requires taste or judgment, it belongs as a plan-only unsafe suggestion, not as an automatic apply.
