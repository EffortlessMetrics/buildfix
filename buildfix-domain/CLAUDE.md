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
    fn fix_key(&self) -> &'static str;
    fn plan(&self, ctx: &PlanContext, repo: &dyn RepoView, receipts: &ReceiptSet) -> Vec<PlannedFix>;
}
```

## Built-in Fixers

| Fixer | Fix Key | Safety | Description |
|-------|---------|--------|-------------|
| `ResolverV2Fixer` | `resolver-v2` | Safe | Sets `[workspace].resolver = "2"` |
| `PathDepVersionFixer` | `path-dep-version` | Safe | Adds version to path dependencies |
| `WorkspaceInheritanceFixer` | `workspace-inheritance` | Safe | Converts deps to `{ workspace = true }` |
| `MsrvNormalizeFixer` | `msrv-normalize` | Guarded | Normalizes crate MSRV to workspace value |

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
- `clean_hashes` - Strip hashes from output for testing
- `policy_caps` - Max operations/files/patch size

### `ReceiptSet`
Helper for accessing receipts filtered by tool/check_id.

## Determinism Mechanisms

- Fixes sorted by `stable_fix_sort_key()` (manifest path + operation type)
- Deterministic UUIDs via `Uuid::new_v5` hashing
- Receipts sorted by path
- Findings sorted by location/tool/check_id

## Invariants

- Never invents values - all data derived from repo state or receipts
- Same inputs always produce byte-identical outputs
- Paths normalized: repo-relative, forward slashes, no leading `./`
