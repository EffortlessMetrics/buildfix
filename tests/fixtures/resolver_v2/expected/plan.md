# buildfix plan

- Ops: 1 (blocked 0)
- Files touched: 1
- Safety: 1 safe, 0 guarded, 0 unsafe
- Inputs: 1

## Ops

### 1. ab9ced5a-84ab-52fb-9080-73eece088fd5

- Safety: `safe`
- Blocked: `false`
- Target: `Cargo.toml`
- Kind: `ensure_workspace_resolver_v2`

Sets [workspace].resolver = "2" for correct feature unification

**Findings**

- `builddiag/workspace.resolver_v2` `not_v2` at Cargo.toml:1

