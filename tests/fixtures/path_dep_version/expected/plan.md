# buildfix plan

- Ops: 1 (blocked 0)
- Files touched: 1
- Inputs: 1

## Ops

### 1. cca08d34-3186-5420-9b9e-56f75695ef63

- Safety: `safe`
- Blocked: `false`
- Target: `crates/a/Cargo.toml`
- Kind: `ensure_path_dep_has_version`

Adds version field to path dependencies for publishability

**Findings**

- `depguard/deps.path_requires_version` `missing_version` at crates/a/Cargo.toml:7

