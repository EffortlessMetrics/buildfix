# buildfix plan

- Ops: 1 (blocked 0)
- Files touched: 1
- Patch bytes: 314
- Inputs: 2

## Ops

### 1. 8a5a8bd3-2fda-5f9e-bbc6-e2ad4d65046c

- Safety: `safe`
- Blocked: `false`
- Target: `crates/crate-a/Cargo.toml`
- Kind: `use_workspace_dependency`

Converts dependency specs to workspace = true inheritance

**Findings**

- `depguard/deps.workspace_inheritance` `should_use_workspace` at crates/crate-a/Cargo.toml:8

