# buildfix plan

- Ops: 1 (blocked 0)
- Files touched: 1
- Safety: 0 safe, 0 guarded, 1 unsafe
- Inputs: 1

## Ops

### 1. 70120bf0-49e2-538b-a79a-3d16cdc86b4c

- Safety: `unsafe`
- Blocked: `false`
- Target: `crates/a/Cargo.toml`
- Kind: `toml_remove`

Removes dependency entries reported as unused

**Findings**

- `cargo-machete/deps.unused_dependency` `unused_dep` at crates/a/Cargo.toml:7

