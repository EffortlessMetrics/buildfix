# buildfix-domain

Deterministic planning logic for buildfix.

This crate decides **what** should change from receipts plus repository state. It does not write files.

## Core concepts

- `Planner`: orchestrates fixers and builds `BuildfixPlan`
- `PlanContext`: repo/artifact paths + planner policy
- `PlannerConfig`: allow/deny rules, safety gates, and operation caps
- `RepoView`: filesystem abstraction for domain logic
- `ReceiptSet`: indexed lookup over loaded receipts

## Built-in fixers

- `cargo.workspace_resolver_v2`
- `cargo.path_dep_add_version`
- `cargo.use_workspace_dependency`
- `cargo.consolidate_duplicate_deps`
- `cargo.remove_unused_deps`
- `cargo.normalize_rust_version`
- `cargo.normalize_edition`
- `cargo.normalize_license`

Use `builtin_fixer_metas()` for stable metadata used by docs/listing surfaces.

## Determinism guarantees

- Stable fixer ordering
- Stable operation sorting
- Deterministic IDs
- Normalized path handling (repo-relative, forward slashes)

## Boundaries

- No direct markdown rendering
- No direct file mutation
- No CLI concerns

This is a support crate for the `buildfix` workspace and may evolve in lockstep with the workspace release train.
