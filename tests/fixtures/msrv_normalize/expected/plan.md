# buildfix plan

- Ops: 1 (blocked 0)
- Files touched: 1
- Safety: 0 safe, 1 guarded, 0 unsafe
- Inputs: 1

## Ops

### 1. 57fb44c4-888e-57e5-8b1b-4374618cfc32

- Safety: `guarded`
- Blocked: `false`
- Target: `crates/a/Cargo.toml`
- Kind: `set_package_rust_version`

Normalizes per-crate MSRV to workspace canonical rust-version

**Findings**

- `builddiag/rust.msrv_consistent` `msrv_mismatch` at crates/a/Cargo.toml:5

