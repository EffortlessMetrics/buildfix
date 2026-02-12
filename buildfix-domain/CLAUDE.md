# buildfix-domain

Core planning logic - decides WHAT should change. Testable independently via the `RepoView` trait abstraction.

## Build & Test

```bash
cargo test -p buildfix-domain
cargo clippy -p buildfix-domain
```

## Architecture

Follows hexagonal architecture: domain logic is isolated from I/O via the `RepoView` port.

### Ports (`ports.rs`)

```rust
trait RepoView {
    fn root(&self) -> &Path;
    fn read_to_string(&self, path: &Path) -> io::Result<String>;
    fn exists(&self, path: &Path) -> bool;
}
```

- `FsRepoView` - Filesystem implementation for production use

### Fixer Trait (`fixers/mod.rs`)

```rust
trait Fixer {
    fn meta(&self) -> FixerMeta;
    fn plan(&self, ctx: &PlanContext, repo: &dyn RepoView, receipts: &ReceiptSet) -> Vec<PlanOp>;
}
```

## Built-in Fixers

| Fixer | Fix Key | Safety | Description |
|-------|---------|--------|-------------|
| `ResolverV2Fixer` | `cargo.workspace_resolver_v2` | Safe | Sets `[workspace].resolver = "2"` |
| `PathDepVersionFixer` | `cargo.path_dep_add_version` | Safe | Adds version to path dependencies |
| `WorkspaceInheritanceFixer` | `cargo.use_workspace_dependency` | Safe | Converts deps to `{ workspace = true }` |
| `MsrvNormalizeFixer` | `cargo.normalize_rust_version` | Guarded | Normalizes crate MSRV to workspace value |
| `EditionUpgradeFixer` | `cargo.normalize_edition` | Guarded | Normalizes crate edition to workspace value |

## Key Types

### `Planner`
Orchestrates all registered fixers and produces a `BuildfixPlan`.

### `PlanContext`
```rust
struct PlanContext {
    repo_root: PathBuf,
    artifacts_dir: PathBuf,
    config: PlannerConfig,
}
```

### `PlannerConfig`
- `allow` / `deny` - Glob patterns for policy filtering
- `params` - Unsafe op params
- `max_ops` / `max_files` / `max_patch_bytes` - Policy caps

### `ReceiptSet`
Helper for accessing receipts filtered by tool/check_id.

## Determinism Mechanisms

- Ops sorted by a stable op sort key (manifest path + rule id + args)
- Deterministic UUIDs via `Uuid::new_v5` hashing
- Receipts sorted by path
- Findings sorted by location/tool/check_id

## Invariants

- Never invents values - all data derived from repo state or receipts
- Same inputs always produce byte-identical outputs
- Paths normalized: repo-relative, forward slashes, no leading `./`
