# buildfix

buildfix is a receipt-driven repair tool for Cargo workspace hygiene.

It consumes sensor receipts from `artifacts/*/report.json`, produces deterministic fix plans, and can apply those fixes with explicit safety gates (`safe`, `guarded`, `unsafe`).

## Workspace crates

| Crate | Responsibility |
| --- | --- |
| `buildfix-types` | Shared DTOs and schema constants for plan/apply/report receipts. |
| `buildfix-receipts` | Tolerant receipt ingestion from `artifacts/*/report.json`. |
| `buildfix-domain` | Planning logic that decides **what** should change. |
| `buildfix-edit` | Edit engine that decides **how** to mutate manifests. |
| `buildfix-render` | Markdown renderers for plan/apply/comment artifacts. |
| `buildfix-core` | Clap-free orchestration pipeline (plan/apply + artifact writing). |
| `buildfix-cli` (package: `buildfix`) | User-facing CLI. |
| `buildfix-bdd` | Cucumber acceptance suite. |
| `xtask` | Developer automation and conformance helpers. |
| `fuzz` | `cargo-fuzz` targets (excluded from workspace members). |

## Quick start

```bash
# Generate plan artifacts
cargo run -p buildfix -- plan

# Dry-run apply (no file writes)
cargo run -p buildfix -- apply

# Apply safe operations
cargo run -p buildfix -- apply --apply

# Include guarded operations
cargo run -p buildfix -- apply --apply --allow-guarded

# Validate receipts and artifacts
cargo run -p buildfix -- validate
```

## Artifacts

`plan` writes to `artifacts/buildfix/`:

- `plan.json`
- `plan.md`
- `comment.md`
- `patch.diff`
- `report.json`
- `extras/buildfix.report.v1.json`

`apply` writes to `artifacts/buildfix/`:

- `apply.json`
- `apply.md`
- `patch.diff`
- `report.json`
- `extras/buildfix.report.v1.json`

## Safety model

- `safe`: deterministic and low-risk; applied with `--apply`
- `guarded`: deterministic but higher-impact; needs `--allow-guarded`
- `unsafe`: requires explicit operator approval and/or params; needs `--allow-unsafe`

Exit codes:

- `0`: success
- `1`: tool/runtime error
- `2`: policy block (for example precondition mismatch or safety gate)

See `docs/safety-model.md` and `docs/architecture.md` for deeper design details.
