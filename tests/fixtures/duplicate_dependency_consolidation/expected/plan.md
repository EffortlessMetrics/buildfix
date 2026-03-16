# buildfix plan

- Ops: 3 (blocked 0)
- Files touched: 3
- Safety: 3 safe, 0 guarded, 0 unsafe
- Inputs: 1

## Ops

### 1. 5bc5e0d5-96a1-53ac-86d3-b960dfb7747e

- Safety: `safe`
- Blocked: `false`
- Target: `Cargo.toml`
- Kind: `ensure_workspace_dependency_version`

Consolidates duplicate dependency versions into workspace.dependencies

**Findings**

- `depguard/deps.duplicate_dependency_versions` `duplicate_version` at crates/a/Cargo.toml:7
- `depguard/deps.duplicate_dependency_versions` `duplicate_version` at crates/b/Cargo.toml:7

### 2. c00514e9-2fc1-535b-81ca-04d634ddb6c7

- Safety: `safe`
- Blocked: `false`
- Target: `crates/a/Cargo.toml`
- Kind: `use_workspace_dependency`

Consolidates duplicate dependency versions into workspace.dependencies

**Findings**

- `depguard/deps.duplicate_dependency_versions` `duplicate_version` at crates/a/Cargo.toml:7

### 3. 31611d71-cd8b-5ec3-980c-2e33851d778e

- Safety: `safe`
- Blocked: `false`
- Target: `crates/b/Cargo.toml`
- Kind: `use_workspace_dependency`

Consolidates duplicate dependency versions into workspace.dependencies

**Findings**

- `depguard/deps.duplicate_dependency_versions` `duplicate_version` at crates/b/Cargo.toml:7

