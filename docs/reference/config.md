# Configuration Schema

Complete reference for `buildfix.toml` configuration.

## File Location

buildfix reads configuration from `buildfix.toml` in the repository root.

## Schema

```toml
[policy]
allow = []                    # Allowlist patterns for fix IDs
deny = []                     # Denylist patterns for fix IDs
allow_guarded = false         # Allow guarded fixes to apply
allow_unsafe = false          # Allow unsafe fixes to apply
allow_dirty = false           # Allow apply on dirty working tree
max_ops = 50                  # Maximum operations in a plan
max_files = 25                # Maximum files touched
max_patch_bytes = 250000      # Maximum patch size in bytes

[backups]
enabled = true                # Create backups before editing
suffix = ".buildfix.bak"      # Backup file suffix

[params]
# key = "value"               # Parameters for unsafe fixes
```

## [policy] Section

### allow

Type: `string[]`
Default: `[]` (all fixes allowed)

Allowlist patterns for fix IDs. If non-empty, only matching fixes are eligible.

```toml
[policy]
allow = [
  "builddiag/workspace.resolver_v2/*",
  "depguard/deps.path_requires_version/*",
]
```

### deny

Type: `string[]`
Default: `[]`

Denylist patterns for fix IDs. Matching fixes are never planned or applied.

```toml
[policy]
deny = [
  "builddiag/rust.msrv_consistent/*",
]
```

### Pattern Syntax

| Pattern | Description | Example |
|---------|-------------|---------|
| `sensor/*` | All fixes from sensor | `depguard/*` |
| `sensor/check_id/*` | All codes for check | `builddiag/workspace.resolver_v2/*` |
| `sensor/check_id/code` | Exact match | `depguard/deps.path_requires_version/missing_version` |

### Evaluation Order

1. Explicit deny wins (if in deny list, blocked)
2. If allow list non-empty, must be in allow list
3. Otherwise, eligible by default

### allow_guarded

Type: `bool`
Default: `false`

Allow guarded fixes to apply. Guarded fixes are deterministic but higher impact.

```toml
[policy]
allow_guarded = true
```

Equivalent CLI: `--allow-guarded`

### allow_unsafe

Type: `bool`
Default: `false`

Allow unsafe fixes to apply. Unsafe fixes require parameters.

```toml
[policy]
allow_unsafe = true
```

Equivalent CLI: `--allow-unsafe`

### allow_dirty

Type: `bool`
Default: `false`

Allow apply when the working tree has uncommitted changes.

```toml
[policy]
allow_dirty = true
```

Equivalent CLI: `--allow-dirty`

### max_ops

Type: `integer`
Default: `50`

Maximum number of operations in a single plan. Exceeding this triggers a policy block (exit 2).

```toml
[policy]
max_ops = 100
```

### max_files

Type: `integer`
Default: `25`

Maximum number of files touched by a plan. Exceeding this triggers a policy block.

```toml
[policy]
max_files = 50
```

### max_patch_bytes

Type: `integer`
Default: `250000`

Maximum size of the generated patch in bytes. Exceeding this triggers a policy block.

```toml
[policy]
max_patch_bytes = 500000
```

## [backups] Section

### enabled

Type: `bool`
Default: `true`

Create backups of files before editing.

```toml
[backups]
enabled = true
```

Backups are stored in `<out-dir>/backups/`.

### suffix

Type: `string`
Default: `".buildfix.bak"`

File suffix for backups.

```toml
[backups]
suffix = ".buildfix.bak"
```

Example: `Cargo.toml` → `Cargo.toml.buildfix.bak`

## [params] Section

Parameters for unsafe fixes. Keys match parameter names expected by specific fixes.

```toml
[params]
rust_version = "1.75"
```

These can also be provided via CLI: `--param rust_version=1.75`

### Known Parameters

| Parameter | Used By | Description |
|-----------|---------|-------------|
| `rust_version` | MSRV normalization | Target rust-version when no workspace standard |

## CLI Overrides

CLI arguments take precedence over config file values:

| Config | CLI Override |
|--------|--------------|
| `allow` | `--allow` |
| `deny` | `--deny` |
| `allow_guarded` | `--allow-guarded` |
| `allow_unsafe` | `--allow-unsafe` |
| `require_clean_hashes` | `--no-clean-hashes` |

## Example Configurations

### Conservative (Minimal Fixes)

```toml
[policy]
allow = [
  "builddiag/workspace.resolver_v2/*",
]
deny = []
allow_guarded = false
allow_unsafe = false
allow_dirty = false
max_ops = 10
max_files = 5
max_patch_bytes = 10000

[backups]
enabled = true
```

### Standard

```toml
[policy]
allow = []
deny = []
allow_guarded = false
allow_unsafe = false
allow_dirty = false
max_ops = 50
max_files = 25
max_patch_bytes = 250000

[backups]
enabled = true
suffix = ".buildfix.bak"
```

### Aggressive (Auto-Fix Everything)

```toml
[policy]
allow = []
deny = []
allow_guarded = true
allow_unsafe = false
allow_dirty = false
max_ops = 200
max_files = 100
max_patch_bytes = 1000000

[backups]
enabled = true
```

## See Also

- [How to Configure buildfix](../how-to/configure.md)
- [Fix Catalog](fixes.md) — Fix keys for allow/deny
- [CLI Reference](cli.md) — Command-line options
