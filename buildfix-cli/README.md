# buildfix

Receipt-driven repair tool for Cargo workspace hygiene. Consumes sensor receipts and emits deterministic repair plans.

## Installation

```bash
cargo install --path buildfix-cli
```

## Commands

### `buildfix plan`
Generate a repair plan from sensor receipts.

```bash
buildfix plan [OPTIONS]
  --repo-root <PATH>         # Repository root (default: cwd)
  --artifacts-dir <PATH>     # Receipts location (default: artifacts/)
  --out-dir <PATH>           # Output directory (default: artifacts/buildfix/)
  --allow <PATTERN>          # Allow glob patterns
  --deny <PATTERN>           # Deny glob patterns
  --param <KEY=VALUE>        # Params for unsafe ops (repeatable)
  --max-ops <N>              # Max operations per plan
  --max-files <N>            # Max files to modify
  --max-patch-bytes <N>      # Max patch size
  --git-head-precondition    # Include git HEAD SHA check
```

Outputs: `plan.json`, `plan.md`, `patch.diff`, `report.json`

### `buildfix apply`
Apply an existing plan.

```bash
buildfix apply [OPTIONS]
  --repo-root <PATH>         # Repository root
  --out-dir <PATH>           # Plan location
  --apply                    # Write changes (dry-run by default)
  --allow-guarded            # Include guarded fixes
  --allow-unsafe             # Include unsafe fixes
  --allow-dirty              # Allow dirty working tree
  --param <KEY=VALUE>        # Params for unsafe ops (repeatable)
```

Outputs: `apply.json`, `apply.md`, `patch.diff`

### `buildfix explain`
Explain a fix by key or ID.

```bash
buildfix explain <FIX_KEY_OR_ID>
buildfix explain resolver-v2
buildfix explain path-dep-version
```

### `buildfix list-fixes`
List known fixes and policy keys.

```bash
buildfix list-fixes
buildfix list-fixes --format json
```

### `buildfix validate`
Validate receipts and buildfix artifacts against embedded schemas.

```bash
buildfix validate
```

## Configuration

Optional `buildfix.toml` in repo root:

```toml
[policy]
allow = ["builddiag/workspace.resolver_v2/*"]
deny = ["builddiag/rust.msrv_consistent/*"]
max_ops = 50

[backups]
enabled = true
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Tool error (I/O, parse) |
| 2 | Policy block (precondition, safety gate) |

This crate is part of the [buildfix](https://github.com/EffortlessMetrics/buildfix) workspace.
