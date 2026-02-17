# xtask

Developer task runner for buildfix workspace maintenance.

This crate provides utility commands for schema IDs, artifact bootstrapping, fixture blessing, validation, and conformance checks.

## Commands

- `xtask print-schemas`
- `xtask init-artifacts [--dir artifacts]`
- `xtask bless-fixtures`
- `xtask validate`
- `xtask conform --artifacts-dir ... [--golden-dir ...] [--contracts-dir ...]`

## What `conform` checks

- Schema validity for `report.json`
- Required field presence
- Optional determinism check against golden artifacts

This crate is internal to the workspace (`publish = false`).
