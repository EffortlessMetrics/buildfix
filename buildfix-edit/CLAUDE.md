# buildfix-edit

TOML editing engine - decides HOW to apply operations. Uses `toml_edit` for format-preserving transformations.

## Build & Test

```bash
cargo test -p buildfix-edit
cargo clippy -p buildfix-edit
```

## Key Functions

### `attach_preconditions(plan, repo_root, options) -> Result<BuildfixPlan>`
Adds `FileExists` + `FileSha256` preconditions to each fix. Optionally captures git HEAD SHA.

### `preview_patch(plan, repo_root) -> Result<String>`
Generates unified diff without writing to disk.

### `apply_plan(plan, repo_root, options) -> Result<ExecuteOutcome>`
Executes plan in-memory or to disk with optional backups.

### `check_policy_block(outcome) -> Option<PolicyBlockError>`
Returns error if any fix was blocked by policy.

## Types

### `ApplyOptions`
```rust
struct ApplyOptions {
    dry_run: bool,           // Don't write to disk
    allow_guarded: bool,     // Apply guarded fixes
    allow_unsafe: bool,      // Apply unsafe fixes
    backup_dir: Option<PathBuf>,
}
```

### `ExecuteOutcome`
- `before` / `after` - Content maps for diffing
- `results` - Per-fix outcomes
- `summary` - Counts of attempted/applied/skipped/failed

## Operation Implementations

Each `Operation` variant has a corresponding TOML transformation:

| Operation | Transformation |
|-----------|----------------|
| `EnsureWorkspaceResolverV2` | Sets `doc["workspace"]["resolver"] = "2"` |
| `SetPackageRustVersion` | Sets `doc["package"]["rust-version"]` |
| `EnsurePathDepHasVersion` | Adds version to path dep inline/table |
| `UseWorkspaceDependency` | Converts to `{ workspace = true }` inline table |

## Policy Enforcement

1. **Glob matching** - Simple `*` and `?` patterns for allow/deny lists
2. **Safety gates** - Safe always allowed; Guarded/Unsafe require flags
3. **Precondition checks** - FileExists, FileSha256, GitHeadSha validation
4. **Backup creation** - Stored alongside file or in dedicated backup_dir

## Error Types

- `EditError` - I/O, parse, or validation failures
- `PolicyBlockError` - Precondition or safety gate denial (exit code 2)

## Invariants

- Never writes without matching preconditions
- TOML formatting preserved (comments, whitespace, ordering)
- Backup created before any file modification
