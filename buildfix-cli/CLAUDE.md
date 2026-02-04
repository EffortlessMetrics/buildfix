# buildfix-cli

CLI entry point wiring clap + all modules.

## Build & Run

```bash
cargo build -p buildfix
cargo run -p buildfix -- plan
cargo run -p buildfix -- apply
cargo run -p buildfix -- explain resolver-v2
```

## Subcommands

### `plan`
Generate a repair plan from sensor receipts.

```bash
buildfix plan [OPTIONS]
  --repo-root <PATH>       # Repository root (default: cwd)
  --artifacts-dir <PATH>   # Receipts location (default: artifacts/)
  --out-dir <PATH>         # Output directory (default: artifacts/buildfix/)
  --allow <PATTERN>        # Allow glob patterns
  --deny <PATTERN>         # Deny glob patterns
  --max-ops <N>            # Max operations per plan
  --max-files <N>          # Max files to modify
  --max-patch-bytes <N>    # Max patch size
  --no-clean-hashes        # Keep SHA hashes in output
  --git-head-precondition  # Include git HEAD SHA check
```

**Outputs:** `plan.json`, `plan.md`, `patch.diff`, `report.json`

### `apply`
Apply an existing plan to the repository.

```bash
buildfix apply [OPTIONS]
  --repo-root <PATH>       # Repository root
  --out-dir <PATH>         # Plan location
  --apply                  # Actually write changes (dry-run by default)
  --allow-guarded          # Include guarded fixes
  --allow-unsafe           # Include unsafe fixes
  --allow-dirty            # Allow dirty working tree
```

**Outputs:** `apply.json`, `apply.md`, `patch.diff`

### `explain`
Explain a fix by key or ID.

```bash
buildfix explain <FIX_KEY_OR_ID>
```

**Outputs:** Safety class, description, triggers, remediation steps

## Configuration File

Optional `buildfix.toml` in repo root:

```toml
[policy]
allow = ["Cargo.toml", "crates/*/Cargo.toml"]
deny = ["vendor/*"]
max_ops = 50
max_files = 25
max_patch_bytes = 250000

[backups]
enabled = true
suffix = ".buildfix.bak"
```

CLI flags override config file values.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Tool error (I/O, parse failures) |
| 2 | Policy block (precondition failure, safety gate denial) |

## Module Structure

- `main.rs` - Clap command definitions and dispatch
- `config.rs` - `buildfix.toml` loading and merging
- `explain.rs` - Fix explanation registry
