# buildfix-core

Embeddable, clap-free core pipeline for buildfix.

This crate orchestrates plan/apply workflows without owning CLI parsing. It is designed for embedding in other binaries and services.

## What this crate owns

- Pipeline orchestration for plan/apply
- Artifact writing helpers
- I/O abstraction through port traits
- Default filesystem/shell adapters

## Public entry points

- `run_plan(settings, receipts_port, git, tool)`
- `write_plan_artifacts(outcome, out_dir, writer)`
- `run_apply(settings, git, tool)`
- `write_apply_artifacts(outcome, out_dir, writer)`

## Port traits

Defined in `ports`:

- `ReceiptSource`
- `GitPort`
- `WritePort`

Default adapters in `adapters`:

- `FsReceiptSource`
- `ShellGitPort`
- `FsWritePort`
- `InMemoryReceiptSource`

## Boundaries

- Uses `buildfix-domain` to decide what to fix
- Uses `buildfix-edit` to execute edits
- Uses `buildfix-render` to build markdown artifacts
- Uses `buildfix-types` for wire/domain data models

This crate is internal to the workspace (`publish = false`).
