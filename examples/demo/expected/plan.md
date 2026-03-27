# buildfix plan

- Ops: 8 (blocked 0)
- Files touched: 4
- Patch bytes: 1282
- Safety: 8 safe, 0 guarded, 0 unsafe
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

### 2. 5bc5e0d5-96a1-53ac-86d3-b960dfb7747e

- Safety: `safe`
- Blocked: `false`
- Target: `Cargo.toml`
- Kind: `ensure_workspace_dependency_version`

Consolidates duplicate dependency versions into workspace.dependencies

**Findings**

- `depguard/deps.duplicate_dependency_versions` `duplicate_version` at crates/api/Cargo.toml:8
- `depguard/deps.duplicate_dependency_versions` `duplicate_version` at crates/cli/Cargo.toml:9
- `depguard/deps.duplicate_dependency_versions` `duplicate_version` at crates/core/Cargo.toml:7

### 3. b2ed5697-1764-5087-92f5-80ab0f3dc514

- Safety: `safe`
- Blocked: `false`
- Target: `crates/api/Cargo.toml`
- Kind: `use_workspace_dependency`

Consolidates duplicate dependency versions into workspace.dependencies

**Findings**

- `depguard/deps.duplicate_dependency_versions` `duplicate_version` at crates/api/Cargo.toml:8

### 4. a1db9461-7870-5e9a-9185-6368abc90128

- Safety: `safe`
- Blocked: `false`
- Target: `crates/cli/Cargo.toml`
- Kind: `use_workspace_dependency`

Consolidates duplicate dependency versions into workspace.dependencies

**Findings**

- `depguard/deps.duplicate_dependency_versions` `duplicate_version` at crates/cli/Cargo.toml:9

### 5. 99bc5f7e-860f-510a-a560-7a4dfa2406c3

- Safety: `safe`
- Blocked: `false`
- Target: `crates/core/Cargo.toml`
- Kind: `use_workspace_dependency`

Consolidates duplicate dependency versions into workspace.dependencies

**Findings**

- `depguard/deps.duplicate_dependency_versions` `duplicate_version` at crates/core/Cargo.toml:7

### 6. 988799d6-ecd7-5ffc-9ee5-9452b50deb91

- Safety: `safe`
- Blocked: `false`
- Target: `crates/api/Cargo.toml`
- Kind: `ensure_path_dep_has_version`

Adds version field to path dependencies for publishability

**Findings**

- `depguard/deps.path_requires_version` `missing_version` at crates/api/Cargo.toml:7

### 7. 1badf50b-4d2c-5914-932d-98822b487211

- Safety: `safe`
- Blocked: `false`
- Target: `crates/cli/Cargo.toml`
- Kind: `ensure_path_dep_has_version`

Adds version field to path dependencies for publishability

**Findings**

- `depguard/deps.path_requires_version` `missing_version` at crates/cli/Cargo.toml:7
- `depguard/deps.path_requires_version` `missing_version` at crates/cli/Cargo.toml:8

### 8. d28145dd-e099-521b-8914-67611b32d5cc

- Safety: `safe`
- Blocked: `false`
- Target: `crates/cli/Cargo.toml`
- Kind: `ensure_path_dep_has_version`

Adds version field to path dependencies for publishability

**Findings**

- `depguard/deps.path_requires_version` `missing_version` at crates/cli/Cargo.toml:7
- `depguard/deps.path_requires_version` `missing_version` at crates/cli/Cargo.toml:8

