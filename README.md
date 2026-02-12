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

# Apply changes (safe ops only by default)
cargo run -p buildfix -- apply --apply

# Allow guarded ops (e.g., MSRV normalization)
cargo run -p buildfix -- apply --apply --allow-guarded

# Provide params for unsafe ops
cargo run -p buildfix -- apply --apply --allow-unsafe --param rust_version=1.75

# Validate receipts and artifacts
cargo run -p buildfix -- validate
```

## Privilege posture

buildfix is read-only by default and never reaches out to the network.

| Capability | Default | Gate |
|------------|---------|------|
| Read repo files | yes | — |
| Write repo files | **no** | `--apply` |
| Network access | **no** | — (none) |
| Code execution | **no** | — (none) |

### Recommended CI lanes

| Lane | Command | Safety |
|------|---------|--------|
| CI (safe only) | `buildfix plan && buildfix apply --apply` | safe ops auto-applied |
| Maintainer | `buildfix apply --apply --allow-guarded` | includes guarded ops |
| Expert | `buildfix apply --apply --allow-unsafe --param key=val` | all ops with params |

See [`docs/safety-model.md`](docs/safety-model.md) for the full safety model.

## Documentation

Full documentation is in [`docs/`](docs/index.md):

- **[Tutorials](docs/tutorials/)** — Getting started, your first fix
- **[How-To Guides](docs/how-to/)** — Configure, integrate CI/CD, troubleshoot, extend
- **[Reference](docs/reference/)** — CLI, fixes, config schema, output formats
- **[Explanation](docs/)** — Architecture, safety model, design rationale

## Notes

- This workspace is designed to be integrated under a larger "director" system.
- Dependencies are not vendored; a normal Cargo environment with registry access is expected.
