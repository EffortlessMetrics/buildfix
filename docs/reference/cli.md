# CLI Reference

Complete reference for buildfix commands and options.

## Synopsis

```
buildfix <COMMAND>

Commands:
  plan      Generate a deterministic fix plan from receipts
  apply     Apply an existing plan (default: dry-run)
  explain   Explain what a fix does
  help      Print help
```

## buildfix plan

Generate a deterministic fix plan from sensor receipts.

```
buildfix plan [OPTIONS]
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--repo-root <PATH>` | `.` | Repository root directory |
| `--artifacts-dir <PATH>` | `<repo-root>/artifacts` | Directory containing sensor receipts |
| `--out-dir <PATH>` | `<artifacts-dir>/buildfix` | Output directory for plan artifacts |
| `--allow <PATTERN>` | | Allowlist patterns for fix IDs (repeatable) |
| `--deny <PATTERN>` | | Denylist patterns for fix IDs (repeatable) |
| `--max-ops <N>` | `50` | Maximum operations in plan |
| `--max-files <N>` | `25` | Maximum files touched |
| `--max-patch-bytes <N>` | `250000` | Maximum patch size in bytes |
| `--no-clean-hashes` | `false` | Disable SHA256 preconditions (not recommended) |
| `--git-head-precondition` | `false` | Include git HEAD SHA in preconditions |

### Outputs

| File | Description |
|------|-------------|
| `plan.json` | Machine-readable plan (buildfix.plan.v1 schema) |
| `plan.md` | Human-readable summary |
| `patch.diff` | Unified diff preview of all changes |
| `report.json` | Cockpit-compatible receipt envelope |

### Examples

```bash
# Basic plan
buildfix plan

# Custom paths
buildfix plan --repo-root /path/to/repo --out-dir /tmp/buildfix

# Allow only specific fixes
buildfix plan --allow "depguard/*" --deny "builddiag/rust.msrv_consistent/*"
```

## buildfix apply

Apply an existing plan to the repository.

```
buildfix apply [OPTIONS]
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--repo-root <PATH>` | `.` | Repository root directory |
| `--out-dir <PATH>` | `<repo-root>/artifacts/buildfix` | Directory containing plan.json |
| `--apply` | `false` | Actually write changes (otherwise dry-run) |
| `--allow-guarded` | `false` | Allow guarded fixes to apply |
| `--allow-unsafe` | `false` | Allow unsafe fixes to apply (requires params) |
| `--allow-dirty` | `false` | Allow apply on dirty working tree |

### Behavior

Without `--apply`:
- Validates preconditions
- Generates apply artifacts
- Does NOT write to repo files

With `--apply`:
- Verifies all file hashes match plan
- Creates backups in `<out-dir>/backups/`
- Applies changes atomically
- Records results in apply.json

### Outputs

| File | Description |
|------|-------------|
| `apply.json` | Execution record (buildfix.apply.v1 schema) |
| `apply.md` | Human-readable summary |
| `patch.diff` | Actual patch applied (may differ from plan preview) |
| `report.json` | Updated cockpit receipt |
| `backups/` | Pre-edit file backups |

### Examples

```bash
# Dry-run (validate only)
buildfix apply

# Apply safe fixes
buildfix apply --apply

# Include guarded fixes
buildfix apply --apply --allow-guarded

# Include unsafe fixes (when params provided)
buildfix apply --apply --allow-unsafe
```

## buildfix explain

Display detailed information about a fix.

```
buildfix explain <FIX_KEY>
```

### Arguments

| Argument | Description |
|----------|-------------|
| `FIX_KEY` | Fix key or fix ID to explain |

### Fix Keys

Lookup supports multiple formats:

| Format | Example |
|--------|---------|
| Short key | `resolver-v2` |
| Fix ID | `cargo.workspace_resolver_v2` |
| Partial ID | `workspace_resolver_v2` |
| With underscores | `resolver_v2` |

### Available Fixes

| Key | Fix ID | Safety |
|-----|--------|--------|
| `resolver-v2` | `cargo.workspace_resolver_v2` | Safe |
| `path-dep-version` | `cargo.path_dep_add_version` | Safe |
| `workspace-inheritance` | `cargo.use_workspace_dependency` | Safe |
| `msrv` | `cargo.normalize_rust_version` | Guarded |

### Output

```
================================================================================
FIX: Workspace Resolver V2
================================================================================

Key:     resolver-v2
Fix ID:  cargo.workspace_resolver_v2
Safety:  Safe

DESCRIPTION
--------------------------------------------------------------------------------
Sets `[workspace].resolver = "2"` in the root Cargo.toml.
...

TRIGGERING FINDINGS
--------------------------------------------------------------------------------
This fix is triggered by sensor findings matching:

  - builddiag / workspace.resolver_v2
  - cargo / cargo.workspace.resolver_v2

SAFETY CLASS: Safe
--------------------------------------------------------------------------------
SAFE fixes are fully determined from repo-local truth and have low impact.
They are applied automatically with `buildfix apply --apply`.

SAFETY RATIONALE
--------------------------------------------------------------------------------
This fix is classified as SAFE because:
- It only modifies the resolver field in the workspace table
...

REMEDIATION GUIDANCE
--------------------------------------------------------------------------------
To manually apply this fix, add or update your root Cargo.toml:
...
```

### Examples

```bash
buildfix explain resolver-v2
buildfix explain path-dep-version
buildfix explain cargo.normalize_rust_version
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Log level filter (e.g., `debug`, `info`, `warn`) |

### Logging Examples

```bash
# Debug logging
RUST_LOG=debug buildfix plan

# Info only
RUST_LOG=info buildfix apply --apply

# Component-specific
RUST_LOG=buildfix_domain=debug buildfix plan
```

## Configuration File

buildfix reads `buildfix.toml` from the repository root. See [Configuration Schema](config.md) for details.

CLI options override config file values where applicable.

## See Also

- [Exit Codes](exit-codes.md)
- [Configuration Schema](config.md)
- [Output Schemas](schemas.md)
