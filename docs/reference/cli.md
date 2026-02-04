# CLI Reference

Complete reference for buildfix commands and options.

## Synopsis

```
buildfix <COMMAND>

Commands:
  plan         Generate a deterministic fix plan from receipts
  apply        Apply an existing plan (default: dry-run)
  explain      Explain what a fix does
  list-fixes   List known fixes and their policy keys
  validate     Validate receipts and buildfix artifacts
  help         Print help
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
| `--allow <PATTERN>` | | Allowlist patterns for policy keys (repeatable) |
| `--deny <PATTERN>` | | Denylist patterns for policy keys (repeatable) |
| `--max-ops <N>` | | Maximum operations in plan |
| `--max-files <N>` | | Maximum files touched |
| `--max-patch-bytes <N>` | | Maximum patch size in bytes |
| `--no-clean-hashes` | `false` | Disable SHA256 preconditions (not recommended) |
| `--git-head-precondition` | `false` | Include git HEAD SHA in preconditions |
| `--param <KEY=VALUE>` | | Parameter values for unsafe ops (repeatable) |

Policy keys are derived from receipt triggers as `sensor/check_id/code`. Use `*` wildcards to match multiple codes.

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

# Allow only resolver-v2 and path-dep-version triggers
buildfix plan --allow "builddiag/workspace.resolver_v2/*" --allow "depguard/deps.path_requires_version/missing_version"

# Provide params for unsafe ops
buildfix plan --param rust_version=1.75
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
| `--allow-guarded` | `false` | Allow guarded ops to apply |
| `--allow-unsafe` | `false` | Allow unsafe ops to apply (requires params) |
| `--allow-dirty` | `false` | Allow apply on dirty working tree |
| `--param <KEY=VALUE>` | | Parameter values for unsafe ops (repeatable) |

### Behavior

Without `--apply`:
- Validates preconditions
- Generates apply artifacts
- Does NOT write to repo files

With `--apply`:
- Verifies all file hashes match the plan
- Creates backups in `<out-dir>/backups/`
- Applies changes atomically
- Records results in apply.json

A policy block (allow/deny, safety gate, caps, precondition mismatch, dirty tree) returns exit code `2`.

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

# Apply safe ops
buildfix apply --apply

# Include guarded ops
buildfix apply --apply --allow-guarded

# Include unsafe ops (when params provided)
buildfix apply --apply --allow-unsafe --param version=1.2.3
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

### Output

`buildfix explain` includes policy keys (derived from triggers) that can be used in allow/deny lists.

## buildfix list-fixes

List known fixes and their policy keys.

```
buildfix list-fixes [--format text|json]
```

JSON output includes `policy_keys` for each fix.

## buildfix validate

Validate receipts and buildfix artifacts against embedded schemas.

```
buildfix validate [OPTIONS]
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--repo-root <PATH>` | `.` | Repository root directory |
| `--artifacts-dir <PATH>` | `<repo-root>/artifacts` | Directory containing sensor receipts |
| `--out-dir <PATH>` | `<artifacts-dir>/buildfix` | Directory containing buildfix artifacts |

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
