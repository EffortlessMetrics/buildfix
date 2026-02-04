# How to Configure buildfix

This guide explains how to set up `buildfix.toml` to control which ops are allowed, safety gates, and operational limits.

## Configuration File Location

buildfix looks for `buildfix.toml` in your repository root. Create it if it doesn't exist:

```bash
touch buildfix.toml
```

## Basic Structure

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

[params]
# Parameters for unsafe ops
```

## Allow and Deny Lists

### Deny Specific Ops

Block an op from ever being planned:

```toml
[policy]
deny = [
  "builddiag/rust.msrv_consistent/*",  # Don't touch MSRV
]
```

### Allow Only Specific Ops

If `allow` is non-empty, only listed policy keys are eligible:

```toml
[policy]
allow = [
  "builddiag/workspace.resolver_v2/*",
  "depguard/deps.path_requires_version/*",
]
```

### Pattern Matching

Patterns support wildcards:

| Pattern | Matches |
|---------|---------|
| `sensor/*` | All findings from that sensor |
| `sensor/check_id/*` | All codes for that check |
| `sensor/check_id/code` | Exact match |

Examples:

```toml
[policy]
# All depguard findings
allow = ["depguard/*"]

# Specific check, any code
allow = ["builddiag/workspace.resolver_v2/*"]

# Exact match
deny = ["depguard/deps.workspace_inheritance/not_inherited"]
```

## Safety Gates

### Allow Guarded Ops

Guarded ops are blocked by default. Enable them:

```toml
[policy]
allow_guarded = true
```

Or use the CLI flag:

```bash
buildfix apply --apply --allow-guarded
```

### Allow Unsafe Ops

Unsafe ops require parameters. Enable with caution:

```toml
[policy]
allow_unsafe = true
```

You must also provide required parameters (see below).

### Allow Dirty Working Tree

By default, apply refuses if the working tree is dirty:

```toml
[policy]
allow_dirty = true
```

## Operational Caps

Prevent runaway ops with limits:

```toml
[policy]
max_ops = 50         # Maximum operations in a plan
max_files = 25       # Maximum files touched
max_patch_bytes = 250000  # Maximum patch size (bytes)
```

If limits are exceeded, the plan is blocked (exit 2).

## Backup Configuration

Control backup behavior during apply:

```toml
[backups]
enabled = true          # Create backups before editing
suffix = ".buildfix.bak"  # Backup file suffix
```

Backups are stored in `artifacts/buildfix/backups/`.

## Parameters for Unsafe Ops

Some ops need explicit values. Provide them in config:

```toml
[params]
rust_version = "1.75"  # For MSRV ops without a workspace standard
```

Or via CLI:

```bash
buildfix apply --param rust_version=1.75
```

## CLI Overrides

CLI arguments override config file values:

```bash
# Override allow/deny from command line
buildfix plan --allow "depguard/*" --deny "builddiag/rust.msrv_consistent/*"

# Disable hash preconditions (not recommended)
buildfix plan --no-clean-hashes
```

## Example: Conservative Policy

Only allow resolver-v2 and path-dep-version policy keys:

```toml
[policy]
allow = [
  "builddiag/workspace.resolver_v2/*",
  "depguard/deps.path_requires_version/*",
]
deny = []
allow_guarded = false
allow_unsafe = false
allow_dirty = false

max_ops = 20
max_files = 10
max_patch_bytes = 50000

[backups]
enabled = true
```

## Example: Aggressive Policy

Allow all ops including guarded:

```toml
[policy]
allow = []  # Empty = all allowed
deny = []
allow_guarded = true
allow_unsafe = false  # Still require params for unsafe
allow_dirty = false

max_ops = 100
max_files = 50
max_patch_bytes = 500000

[backups]
enabled = true
```

## See Also

- [Configuration Schema Reference](../reference/config.md)
- [Fix Catalog](../reference/fixes.md) — Fix keys for allow/deny lists
- [Troubleshoot Blocked Fixes](troubleshoot.md)
