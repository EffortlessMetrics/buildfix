# buildfix plan

- Ops: 3 (blocked 0)
- Files touched: 2
- Inputs: 2

## Ops

### 1. ab9ced5a-84ab-52fb-9080-73eece088fd5

- Safety: `safe`
- Blocked: `false`
- Target: `Cargo.toml`
- Kind: `ensure_workspace_resolver_v2`

Sets [workspace].resolver = "2" for correct feature unification

**Findings**

- `builddiag/workspace.resolver_v2` `not_v2` at Cargo.toml:1

### 2. cca08d34-3186-5420-9b9e-56f75695ef63

- Safety: `safe`
- Blocked: `false`
- Target: `crates/a/Cargo.toml`
- Kind: `ensure_path_dep_has_version`

Adds version field to path dependencies for publishability

**Findings**

- `depguard/deps.path_requires_version` `missing_version` at crates/a/Cargo.toml:7

### 3. 482bf98c-2e65-5de5-a8ef-5fdf1e64c52f

- Safety: `safe`
- Blocked: `false`
- Target: `crates/a/Cargo.toml`
- Kind: `use_workspace_dependency`

Converts dependency specs to workspace = true inheritance

**Findings**

- `depguard/deps.workspace_inheritance` `should_use_workspace` at crates/a/Cargo.toml:8

