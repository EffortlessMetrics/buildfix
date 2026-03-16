# buildfix plan

- Ops: 1 (blocked 0)
- Files touched: 1
- Safety: 1 safe, 0 guarded, 0 unsafe
- Inputs: 1

## Ops

### 1. 482bf98c-2e65-5de5-a8ef-5fdf1e64c52f

- Safety: `safe`
- Blocked: `false`
- Target: `crates/a/Cargo.toml`
- Kind: `use_workspace_dependency`

Converts dependency specs to workspace = true inheritance

**Findings**

- `depguard/deps.workspace_inheritance` `should_use_workspace` at crates/a/Cargo.toml:7

