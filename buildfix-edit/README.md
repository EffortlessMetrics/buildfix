# buildfix-edit

TOML editing engine for buildfix. This crate decides *how* to apply operations using `toml_edit` for format-preserving transformations.

## Key Functions

### `attach_preconditions(plan, repo_root, options)`
Adds SHA256 preconditions for each touched file and optional git HEAD SHA.

### `preview_patch(plan, repo_root)`
Generates unified diff without writing to disk.

### `apply_plan(plan, repo_root, options)`
Executes plan in-memory or to disk with optional backups.

### `check_policy_block(outcome)`
Returns error if any op was blocked by policy.

## Operation Implementations

| Rule ID | TOML Transformation |
|---------|---------------------|
| `ensure_workspace_resolver_v2` | Sets `[workspace].resolver = "2"` |
| `set_package_rust_version` | Sets `[package].rust-version` |
| `ensure_path_dep_has_version` | Adds version to path dependency |
| `use_workspace_dependency` | Converts to `{ workspace = true }` |

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
    backup_enabled: bool,
    backup_dir: Option<PathBuf>,
    backup_suffix: String,
    params: HashMap<String, String>,
}
```

### `ExecuteOutcome`
- `before` / `after` content maps
- `results` per op
- `summary` with counts

This crate is part of the [buildfix](https://github.com/EffortlessMetrics/buildfix) workspace.
