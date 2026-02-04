# xtask

Build helpers for workspace maintenance.

## Build & Run

```bash
cargo xtask print-schemas
cargo xtask init-artifacts
cargo xtask init-artifacts --dir my-artifacts
```

## Commands

### `print-schemas`
Prints all schema version identifiers used by buildfix artifacts.

```bash
cargo xtask print-schemas
# Output:
# BUILDFIX_PLAN_V1
# BUILDFIX_APPLY_V1
# BUILDFIX_REPORT_V1
```

### `init-artifacts`
Creates the expected artifact directory structure.

```bash
cargo xtask init-artifacts [--dir <PATH>]
```

Creates:
```
artifacts/
  buildscan/
  builddiag/
  depguard/
  buildfix/
```

## Adding New Commands

1. Add variant to the `Cli` enum in `main.rs`
2. Implement handler function
3. Add match arm in `main()`

## Convention

The xtask pattern uses a separate crate for build automation, invoked via `cargo xtask <cmd>`. This avoids polluting the main build with dev-only dependencies.
