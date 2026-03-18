# CLAUDE.md

Shared contracts for buildfix fixer microcrates.

## Build & Test

```bash
cargo build -p buildfix-fixer-api
cargo test -p buildfix-fixer-api
```

## Description

Defines the `Fixer` trait and supporting types that all fixer implementations must implement. Also provides `ReceiptSet` for querying findings.

## Key Types

- `Fixer` — trait implementing `meta()` and `plan()`
- `FixerMeta` — metadata: fix_key, description, safety, consumes_sensors, consumes_check_ids
- `RepoView` — trait for reading repository files
- `PlannerConfig` — configuration: allow, deny, allow_guarded, allow_unsafe, max_ops, max_files, params
- `PlanContext` — context passed to fixers: repo_root, artifacts_dir, config
- `ReceiptSet` — in-memory queryable set of loaded receipts
- `ReceiptRecord` — individual receipt with sensor_id, path, envelope
- `FindingRef` — reference to a finding with source, check_id, code, path, line, fingerprint

## Key Methods

- `ReceiptSet::matching_findings()` — query findings by tool prefixes, check_ids, codes
- `ReceiptSet::matching_findings_with_data()` — same but includes finding data payload
