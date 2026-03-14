# buildfix (`buildfix-cli`)

CLI frontend for the buildfix repair pipeline.

This crate owns command-line UX: argument parsing, config merge, exit-code handling, schema validation commands, and fix explanation/listing.

## Install

```bash
cargo install buildfix --locked
```

## Commands

- `buildfix plan`: load receipts and produce plan artifacts
- `buildfix apply`: apply `plan.json` (dry-run unless `--apply`)
- `buildfix explain <fix-key>`: show fix rationale, safety, and policy keys
- `buildfix list-fixes [--format text|json]`: enumerate built-in fixes
- `buildfix validate`: validate receipts and buildfix artifacts against schemas

## Key options

`plan` supports policy and precondition controls such as:

- `--allow`, `--deny`
- `--max-ops`, `--max-files`, `--max-patch-bytes`
- `--git-head-precondition`
- `--no-clean-hashes`
- `--param key=value`
- `--mode standalone|cockpit`

`apply` supports execution and safety controls such as:

- `--apply`
- `--allow-guarded`
- `--allow-unsafe`
- `--allow-dirty`
- `--auto-commit [--commit-message ...]`
- `--param key=value`
- `--mode standalone|cockpit`

## Config file

The CLI merges `buildfix.toml` with CLI flags (CLI wins). Supported sections:

- `[policy]`
- `[backups]`
- `[commit]`
- `[params]`

## Artifact outputs

Plan run outputs:

- `plan.json`, `plan.md`, `comment.md`, `patch.diff`, `report.json`, `extras/buildfix.report.v1.json`

Apply run outputs:

- `apply.json`, `apply.md`, `patch.diff`, `report.json`, `extras/buildfix.report.v1.json`

## Exit codes

- `0`: success
- `1`: internal/tool error
- `2`: policy block (safety gate, precondition mismatch, denied fix)
