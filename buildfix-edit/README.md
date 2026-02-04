# buildfix-edit

TOML editing engine for buildfix. This crate decides *how* to apply operations using `toml_edit` for format-preserving transformations.

## Key Functions

### `attach_preconditions(plan, repo_root, options)`
Adds `FileExists` + `FileSha256` preconditions to each fix. Optionally captures git HEAD SHA.

### `preview_patch(plan, repo_root)`
Generates unified diff without writing to disk.

### `apply_plan(plan, repo_root, options)`
Executes plan in-memory or to disk with optional backups.

### `check_policy_block(outcome)`
Returns error if any fix was blocked by policy.

## Operation Implementations

| Operation | TOML Transformation |
|-----------|---------------------|
| `EnsureWorkspaceResolverV2` | Sets `[workspace].resolver = "2"` |
| `SetPackageRustVersion` | Sets `[package].rust-version` |
| `EnsurePathDepHasVersion` | Adds version to path dependency |
| `UseWorkspaceDependency` | Converts to `{ workspace = true }` |

## Policy Enforcement

- Glob matching for allow/deny patterns
- Safety class gates (Safe auto-allowed, Guarded/Unsafe require flags)
- Precondition validation (file hashes, git HEAD)
- Backup creation before modifications

## Types

### `ApplyOptions`
```rust
struct ApplyOptions {
    dry_run: bool,
    allow_guarded: bool,
    allow_unsafe: bool,
    backup_dir: Option<PathBuf>,
}
```

### `ExecuteOutcome`
- `before` / `after` content maps
- `results` per fix
- `summary` with counts

This crate is part of the [buildfix](https://github.com/EffortlessMetrics/buildfix) workspace.
