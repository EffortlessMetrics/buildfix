# buildfix (architecture-kit workspace)

This repository is a multi-crate workspace for **buildfix**: a receipt-driven repair tool aimed at common Cargo workspace hygiene issues.

The intent is mechanical:

- Sensors (buildscan/builddiag/depguard) emit `artifacts/<sensor>/report.json`.
- `buildfix plan` reads receipts and produces:
  - `artifacts/buildfix/plan.json`
  - `artifacts/buildfix/plan.md`
  - `artifacts/buildfix/patch.diff` (preview)
  - `artifacts/buildfix/report.json` (cockpit-compatible)
- `buildfix apply` reads `plan.json` and (optionally) writes edits back to the repo.

## Crates

- `buildfix-types` — schemaed DTOs (plan/apply/report, operations, receipt envelope).
- `buildfix-receipts` — tolerant receipt loader (`artifacts/*/report.json`).
- `buildfix-domain` — planner: receipts → deterministic planned operations.
- `buildfix-edit` — editor: operations → `toml_edit` mutations + unified diffs.
- `buildfix-render` — markdown rendering for artifacts.
- `buildfix` (in `buildfix-cli`) — the CLI.
- `buildfix-bdd` — cucumber harness (scenario-style tests).
- `xtask` — small workspace helper.

## Quick start

```bash
# Plan
cargo run -p buildfix -- plan

# Dry-run apply (generates apply.json/apply.md + patch.diff but does not write)
cargo run -p buildfix -- apply

# Apply changes (safe fixes only by default)
cargo run -p buildfix -- apply --apply

# Allow guarded fixes (e.g., MSRV normalization)
cargo run -p buildfix -- apply --apply --allow-guarded
```

## Documentation

Full documentation is in [`docs/`](docs/index.md):

- **[Tutorials](docs/tutorials/)** — Getting started, your first fix
- **[How-To Guides](docs/how-to/)** — Configure, integrate CI/CD, troubleshoot, extend
- **[Reference](docs/reference/)** — CLI, fixes, config schema, output formats
- **[Explanation](docs/)** — Architecture, safety model, design rationale

## Notes

- This workspace is designed to be integrated under a larger "director" system.
- Dependencies are not vendored; a normal Cargo environment with registry access is expected.
