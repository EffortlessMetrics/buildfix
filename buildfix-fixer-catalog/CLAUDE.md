# CLAUDE.md

Canonical built-in fixer metadata for buildfix feature flagging and registration.

## Build & Test

```bash
cargo build -p buildfix-fixer-catalog
cargo test -p buildfix-fixer-catalog
```

## Description

Central registry of all built-in fixers with their metadata (keys, safety, triggers). Used for feature flagging and CLI registration.

## Key Types

- `TriggerPattern` — sensor + check_id + optional code pattern
- `FixerCatalogEntry` — CLI key, fix_id, safety, triggers

## Enabled Fixers

| Key | Fix ID | Safety | Triggers |
|-----|--------|--------|----------|
| resolver-v2 | cargo.workspace_resolver_v2 | Safe | builddiag, cargo |
| path-dep-version | cargo.path_dep_add_version | Safe | depguard |
| workspace-inheritance | cargo.use_workspace_dependency | Safe | depguard |
| duplicate-deps | cargo.consolidate_duplicate_deps | Safe | depguard |
| remove-unused-deps | cargo.remove_unused_deps | Unsafe | cargo-udeps, machete |
| msrv | cargo.normalize_rust_version | Guarded | builddiag, cargo |
| edition | cargo.normalize_edition | Guarded | builddiag, cargo |
| license | cargo.normalize_license | Guarded | cargo-deny |

## Key Functions

- `enabled_fix_catalog()` — all enabled entries
- `enabled_fix_ids()` — just fix_id strings
- `enabled_fix_keys()` — just CLI keys
- `lookup_fix()` — find by key, fix_id, or suffix (case-insensitive, underscores normalized)

## Features

Each fixer is behind a feature flag (e.g., `fixer-resolver-v2`). Default enables all fixers.
