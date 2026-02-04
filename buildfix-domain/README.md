# buildfix-domain

Core planning logic for buildfix. This crate decides *what* should change based on sensor receipts.

## Architecture

Follows hexagonal architecture: domain logic is isolated from I/O via the `RepoView` trait.

### RepoView Port
```rust
trait RepoView {
    fn root(&self) -> &Path;
    fn read_to_string(&self, path: &Path) -> io::Result<String>;
    fn exists(&self, path: &Path) -> bool;
}
```

### Fixer Trait
```rust
trait Fixer {
    fn fix_key(&self) -> &'static str;
    fn plan(&self, ctx: &PlanContext, repo: &dyn RepoView, receipts: &ReceiptSet) -> Vec<PlannedFix>;
}
```

## Built-in Fixers

| Fixer | Fix Key | Safety | Description |
|-------|---------|--------|-------------|
| `ResolverV2Fixer` | `resolver-v2` | Safe | Sets workspace resolver = "2" |
| `PathDepVersionFixer` | `path-dep-version` | Safe | Adds version to path deps |
| `WorkspaceInheritanceFixer` | `workspace-inheritance` | Safe | Converts to workspace = true |
| `MsrvNormalizeFixer` | `msrv-normalize` | Guarded | Normalizes MSRV to workspace |

## Key Types

- `Planner` - Orchestrates all fixers to produce a `BuildfixPlan`
- `PlanContext` - Contains repo_root, artifacts_dir, and config
- `PlannerConfig` - Allow/deny lists, policy caps, clean_hashes flag
- `ReceiptSet` - Helper for accessing receipts by tool/check_id
- `FsRepoView` - Filesystem implementation of `RepoView`

## Determinism

- Fixes sorted by `stable_fix_sort_key()` (manifest path + operation type)
- Deterministic UUIDs via `Uuid::new_v5` hashing
- All collections sorted for byte-stable output

This crate is part of the [buildfix](https://github.com/EffortlessMetrics/buildfix) workspace.
