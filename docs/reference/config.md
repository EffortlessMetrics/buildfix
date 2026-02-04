# Configuration Schema

Complete reference for `buildfix.toml` configuration.

## File Location

buildfix reads configuration from `buildfix.toml` in the repository root.

## Schema

```toml
[policy]
allow = []                    # Allowlist patterns for policy keys
allow_guarded = false         # Allow guarded ops to apply
allow_unsafe = false          # Allow unsafe ops to apply
allow_dirty = false           # Allow apply on dirty working tree
deny = []                     # Denylist patterns for policy keys
max_ops = 50                  # Maximum operations in a plan
max_files = 25                # Maximum files touched
max_patch_bytes = 250000      # Maximum patch size in bytes

[backups]
enabled = true                # Create backups before editing
suffix = ".buildfix.bak"      # Backup file suffix

[params]
# key = "value"               # Parameters for unsafe ops
```

## [policy] Section

### allow

Type: `string[]`
Default: `[]` (all ops allowed)

Allowlist patterns for policy keys. Policy keys are derived from receipt triggers as `sensor/check_id/code`.

```toml
[policy]
allow = [
  "builddiag/workspace.resolver_v2/*",
  "depguard/deps.path_requires_version/missing_version",
]
```

### deny

Type: `string[]`
Default: `[]`

Denylist patterns for policy keys. Matching ops are blocked.

```toml
[policy]
deny = [
  "builddiag/rust.msrv_consistent/*",
]
```

### Pattern Syntax

| Pattern | Description | Example |
|---------|-------------|---------|
| `sensor/*` | All findings from a sensor | `depguard/*` |
| `sensor/check_id/*` | All codes for a check | `builddiag/workspace.resolver_v2/*` |
| `sensor/check_id/code` | Exact match | `depguard/deps.path_requires_version/missing_version` |

### Evaluation Order

1. Explicit deny wins (if in deny list, blocked)
2. If allow list non-empty, must match allow list
3. Otherwise, eligible by default

### allow_guarded

Type: `bool`
Default: `false`

Allow guarded ops to apply. Guarded ops are deterministic but higher impact.

```toml
[policy]
allow_guarded = true
```

Equivalent CLI: `--allow-guarded`

### allow_unsafe

Type: `bool`
Default: `false`

Allow unsafe ops to apply. Unsafe ops require parameters.

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

Parameters for unsafe ops. Keys match parameter names expected by specific ops.

```toml
[params]
rust_version = "1.75"
version = "1.2.3"
```

These can also be provided via CLI: `--param rust_version=1.75`

### Known Parameters

| Parameter | Used By | Description |
|-----------|---------|-------------|
| `rust_version` | MSRV normalization | Target rust-version when no workspace standard |
| `version` | Path dependency version | Version to add when missing |

## CLI Overrides

CLI arguments take precedence over config file values:

| Config | CLI Override |
|--------|--------------|
| `allow` | `--allow` |
| `deny` | `--deny` |
| `allow_guarded` | `--allow-guarded` |
| `allow_unsafe` | `--allow-unsafe` |
| `allow_dirty` | `--allow-dirty` |
| `params` | `--param` |

Plan-only: use `--no-clean-hashes` to disable precondition hashes.

## Example Configurations

### Conservative (Minimal Ops)

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
allow_unsafe = true
allow_dirty = false
max_ops = 200
max_files = 100
max_patch_bytes = 1000000

[backups]
enabled = true
```

## See Also

- [How to Configure buildfix](../how-to/configure.md)
- [Fix Catalog](fixes.md) — Policy keys for allow/deny
- [CLI Reference](cli.md) — Command-line options
