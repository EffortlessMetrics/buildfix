# buildfix

buildfix is a receipt-driven repair tool for Cargo workspace hygiene.

It consumes sensor receipts from `artifacts/*/report.json`, produces deterministic fix plans, and can apply those plans with explicit safety gates (`safe`, `guarded`, `unsafe`).

## Status

### NOW

- 8 built-in fixers with deterministic/TOML/JSON/YAML/anchored-text edit support.
- Receipt-driven planning and safety gates across `safe`, `guarded`, and `unsafe`.
- SHA-256 preconditions, atomic apply path, and optional git-head enforcement.
- CLI command surface includes `plan`, `apply`, `explain`, `list-fixes`, and `validate`.

### NEXT

- Finalize release cut for implemented 0.2.x capability surface (`text_replace_anchored`, JSON/YAML ops, duplicate deps, license normalization, auto-commit).
- Expand contributor guidance for adding per-crate fixers in the modular architecture.
- Keep roadmap, changelog, and docs aligned as release milestones move to LATER→NEXT completion.

### LATER

- Add more provably mechanical file formats and sensors/fixers with strong safety review.

## Architecture at a glance

- Shared contracts: `buildfix-types`, `buildfix-receipts`, `buildfix-hash`
- Policy/domain orchestration: `buildfix-core`, `buildfix-domain`, `buildfix-domain-policy`
- Fixer layer: `buildfix-fixer-api`, `buildfix-fixer-catalog`, `buildfix-fixer-*`
- Edit/reporting: `buildfix-edit`, `buildfix-report`, `buildfix-artifacts`, `buildfix-render`
- Host/runtime: `buildfix-core-runtime`, `buildfix-cli`
- Quality: `buildfix-bdd`, `xtask`, `fuzz`

## Flow

Receipts → normalized findings → cataloged fixers → planned ops → policies/caps/preconditions → deterministic plan/report artifacts → gated apply

## Quick start

```bash
cargo install buildfix --locked
```

```bash
# Generate plan artifacts
cargo run -p buildfix -- plan

# Discover fixes and explainability
cargo run -p buildfix -- list-fixes
cargo run -p buildfix -- explain resolver-v2

# Dry-run apply (no file writes)
cargo run -p buildfix -- apply

# Apply safe operations
cargo run -p buildfix -- apply --apply

# Include guarded operations
cargo run -p buildfix -- apply --apply --allow-guarded

# Include unsafe operations (requires params)
cargo run -p buildfix -- apply --apply --allow-unsafe --param version="1.2.3"

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

## Further reading

- [`docs/index.md`](docs/index.md)
- [`ROADMAP.md`](ROADMAP.md)
- [`CHANGELOG.md`](CHANGELOG.md)
- [`docs/architecture.md`](docs/architecture.md)
- [`docs/reference/fixes.md`](docs/reference/fixes.md)
- [`docs/safety-model.md`](docs/safety-model.md)
- [`docs/reference/cli.md`](docs/reference/cli.md)
- [`docs/adr/README.md`](docs/adr/README.md)
