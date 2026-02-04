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
    fn meta(&self) -> FixerMeta;
    fn plan(&self, ctx: &PlanContext, repo: &dyn RepoView, receipts: &ReceiptSet) -> Vec<PlanOp>;
}
```

## Built-in Fixers

| Fixer | Fix Key | Safety | Description |
|-------|---------|--------|-------------|
| `ResolverV2Fixer` | `cargo.workspace_resolver_v2` | Safe | Sets workspace resolver = "2" |
| `PathDepVersionFixer` | `cargo.path_dep_add_version` | Safe | Adds version to path deps |
| `WorkspaceInheritanceFixer` | `cargo.use_workspace_dependency` | Safe | Converts to workspace = true |
| `MsrvNormalizeFixer` | `cargo.normalize_rust_version` | Guarded | Normalizes MSRV to workspace |

## Key Types

- `Planner` - Orchestrates all fixers to produce a `BuildfixPlan`
- `PlanContext` - Contains repo_root, artifacts_dir, and config
- `PlannerConfig` - Allow/deny lists, policy caps, params
- `ReceiptSet` - Helper for accessing receipts by tool/check_id
- `FsRepoView` - Filesystem implementation of `RepoView`

## Determinism

- Ops sorted by a stable op sort key (manifest path + rule id + args)
- Deterministic UUIDs via `Uuid::new_v5` hashing
- All collections sorted for byte-stable output

This crate is part of the [buildfix](https://github.com/EffortlessMetrics/buildfix) workspace.
